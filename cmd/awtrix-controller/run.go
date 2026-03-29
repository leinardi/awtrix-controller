/*
 * MIT License
 *
 * Copyright (c) 2026 Roberto Leinardi
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

package main

import (
	"context"
	"encoding/json"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/leinardi/awtrix-controller/internal/broker"
	"github.com/leinardi/awtrix-controller/internal/clientstate"
	"github.com/leinardi/awtrix-controller/internal/clock"
	"github.com/leinardi/awtrix-controller/internal/config"
	"github.com/leinardi/awtrix-controller/internal/daynight"
	"github.com/leinardi/awtrix-controller/internal/energysaving"
	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
	"github.com/leinardi/awtrix-controller/internal/notification"
	"github.com/leinardi/awtrix-controller/internal/scheduler"
	"github.com/leinardi/awtrix-controller/internal/settings"
	"github.com/leinardi/awtrix-controller/internal/weather"
	mqtt "github.com/wind-c/comqtt/v2/mqtt"
)

// run is the application entry point called by main. It returns an exit code:
//   - 0: clean shutdown
//   - 1: configuration error
//   - 2: runtime startup error (e.g. port already in use)
//
// All logic lives in runWithContext so that defer statements execute before
// os.Exit is called and so that the shutdown sequence is testable without
// sending real OS signals.
func run() int {
	ctx, stop := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer stop()

	return runWithContext(ctx, os.Args[1:], os.Getenv)
}

// runWithContext wires all application components together and blocks until ctx
// is canceled. It is extracted from run so that tests can inject a
// cancellable context instead of sending real OS signals.
//
//nolint:cyclop,funlen,gocognit,gocyclo,maintidx // startup wiring is inherently long; splitting would obscure the sequence
func runWithContext(ctx context.Context, args []string, getenv func(string) string) int {
	// Step 1: Parse CLI flags.
	flagSet := flag.NewFlagSet("awtrix-controller", flag.ContinueOnError)

	var (
		configPath      string
		logLevelStr     string
		showVersion     bool
		weatherWMO      int
		testNotifParams testNotificationParams
	)

	flagSet.StringVar(&configPath, "config", config.DefaultConfigPath, "Path to YAML config file")
	flagSet.StringVar(
		&configPath,
		"c",
		config.DefaultConfigPath,
		"Path to YAML config file (shorthand)",
	)
	flagSet.StringVar(&logLevelStr, "log-level", "info", "Log verbosity (debug|info|warn|error)")
	flagSet.StringVar(&logLevelStr, "l", "info", "Log verbosity (shorthand)")
	flagSet.BoolVar(&showVersion, "version", false, "Print version and exit")
	flagSet.BoolVar(&showVersion, "v", false, "Print version and exit (shorthand)")
	flagSet.IntVar(&weatherWMO, "weather-wmo", 0,
		"Simulate all forecast points with this WMO code (0 = disabled; debug only)")
	registerTestNotificationFlags(flagSet, &testNotifParams)

	parseErr := flagSet.Parse(args)
	if parseErr != nil {
		return 1
	}

	// Step 2: Env-var fallback for any flag not explicitly set on the CLI.
	visited := make(map[string]bool)

	flagSet.Visit(func(f *flag.Flag) { visited[f.Name] = true })

	if !visited["config"] && !visited["c"] {
		if envVal := getenv("AWTRIX_CONFIG"); envVal != "" {
			configPath = envVal
		}
	}

	flagWasSet := visited["log-level"] || visited["l"]
	logLevel := effectiveLevel(logLevelStr, getenv("AWTRIX_LOG_LEVEL"), flagWasSet)

	testNotif := buildTestNotification(visited, &testNotifParams)

	// Step 3: Handle --version flag.
	if showVersion {
		fmt.Fprintf(
			os.Stdout,
			"%s %s %s\n",
			version,
			commit,
			date,
		)

		return 0
	}

	// Step 4: Initialize structured logger.
	logger.Init(logLevel)

	if testNotif != nil {
		logger.L().Warn("test-notification mode active — sending test notification on device ready")
	}

	// Step 5: Load and validate configuration.
	cfg, loadErr := config.Load(configPath)
	if loadErr != nil {
		logger.L().Error("failed to load configuration", "path", configPath, "err", loadErr)

		return 1
	}

	logger.L().
		Debug("config: effective configuration loaded", "config", config.NewConfigDebugView(cfg))

	// Step 6: Resolve timezone; warn and fall back to system TZ when absent.
	var timezone *time.Location

	if cfg.Timezone == "" {
		logger.L().Warn("no timezone configured; using system timezone")

		timezone = time.Local //nolint:gosmopolitan // intentional system-TZ fallback per SPEC §7
	} else {
		var tzErr error

		timezone, tzErr = time.LoadLocation(cfg.Timezone)
		if tzErr != nil {
			// config.Validate already verified the timezone; this is a defensive guard.
			logger.L().Error("invalid timezone", "timezone", cfg.Timezone, "err", tzErr)

			return 1
		}
	}

	// Step 7: Create shared infrastructure.
	realClock := clock.NewRealClock()
	registry := clientstate.NewRegistry()
	sched := scheduler.New(ctx, realClock)

	// pushSettingsToAll is declared here so it can be captured by the onChange
	// callbacks below; it is assigned after settingsBuilder is created in step 10.
	var pushSettingsToAll func()

	// Step 8: Day/night controller.
	dayNightCtrl := daynight.New(
		cfg.Location,
		timezone,
		realClock,
		sched,
		func(mode daynight.Mode) {
			logger.L().Info("day/night mode changed", "mode", mode.String())
			pushSettingsToAll()
		},
	)

	dnStartErr := dayNightCtrl.Start()
	if dnStartErr != nil {
		logger.L().Error("failed to start day/night controller", "err", dnStartErr)

		return 2
	}

	// Step 9: Energy-saving controller.
	energySavingCtrl := energysaving.New(
		cfg.EnergySaving,
		timezone,
		realClock,
		sched,
		func(active bool) {
			logger.L().Info("energy-saving mode changed", "active", active)
			pushSettingsToAll()
		},
	)

	esStartErr := energySavingCtrl.Start()
	if esStartErr != nil {
		logger.L().Error("failed to start energy-saving controller", "err", esStartErr)

		return 2
	}

	// Step 10: Settings builder and push closures.
	// server is declared here and assigned in step 12; both closures capture it
	// by reference so they always use the assigned value when called.
	settingsBuilder := settings.NewBuilder(cfg.Theme, dayNightCtrl, energySavingCtrl)

	var server *mqtt.Server

	pushSettingsToAll = func() {
		for _, clientID := range registry.ConnectedIDs() {
			pushErr := settings.Push(clientID, server, settingsBuilder.Build())
			if pushErr != nil {
				logger.L().Warn("failed to push settings to client",
					"clientID", clientID,
					"err", pushErr,
				)
			}
		}
	}

	// Step 11: Controller hook.
	// weatherCtrl is declared here so the onDeviceConnected closure can capture
	// it by reference; it is assigned in step 14 after the controller is created.
	// publishNotification is declared here so that onDeviceReady can capture it
	// by reference; it is assigned after broker creation in step 12.
	var weatherCtrl *weather.Controller

	var publishNotification func(clientID string, n *model.Notification) error

	onDeviceConnected := func(clientID string) error {
		settingsErr := settings.Push(clientID, server, settingsBuilder.Build())
		if settingsErr != nil {
			return fmt.Errorf("run: push settings on connect for %s: %w", clientID, settingsErr)
		}

		return nil
	}

	onDeviceReady := func(clientID string) {
		if weatherCtrl != nil {
			weatherCtrl.OnDeviceConnected(clientID)
		}

		if testNotif != nil {
			testErr := publishNotification(clientID, testNotif)
			if testErr != nil {
				logger.L().Warn("test-notification: publish failed",
					"clientID", clientID,
					"err", testErr,
				)
			}
		}
	}

	hook := broker.NewControllerHook(
		cfg.MQTT.Username,
		cfg.MQTT.Password,
		registry,
		onDeviceConnected,
		onDeviceReady,
	)

	// Step 12: Create MQTT broker.
	var brokerErr error

	server, brokerErr = broker.New(cfg, hook)
	if brokerErr != nil {
		logger.L().Error("failed to create MQTT broker", "err", brokerErr)

		return 2
	}

	// publishNotification marshals and publishes a transient notification to a
	// single client on the {clientID}/notify topic (non-retained, QoS 1).
	publishNotification = func(clientID string, n *model.Notification) error {
		payload, marshalErr := json.Marshal(n)
		if marshalErr != nil {
			return fmt.Errorf("run: marshal notification for %s: %w", clientID, marshalErr)
		}

		topic := clientID + "/notify"

		publishErr := server.Publish(topic, payload, false, 1)
		if publishErr != nil {
			return fmt.Errorf("run: publish notification to %s: %w", topic, publishErr)
		}

		return nil
	}

	// Step 13: Start notifiers.
	connectedIDs := registry.ConnectedIDs

	scheduledNotifier := notification.NewScheduledNotifier(
		cfg.ScheduledNotifications,
		timezone,
		realClock,
		sched,
		publishNotification,
	)
	scheduledNotifier.Start(connectedIDs)

	// Step 14: Weather controller (optional).
	if cfg.Weather.Enabled {
		effectiveTimezone := cfg.Timezone

		var fetchFn weather.FetchFunc

		if weatherWMO != 0 {
			logger.L().
				Warn("weather: simulation mode active — real API disabled", "wmo_code", weatherWMO)

			fetchFn = weather.SimulateFetchFunc(weatherWMO)
		} else {
			fetchFn = weather.NewFetcher().Fetch
		}

		pushOverlayToAll := func(overlay model.OverlayEffect) {
			settingsBuilder.SetOverlay(overlay)
			pushSettingsToAll()
		}

		weatherCtrl = weather.NewWithFetchFunc(
			cfg.Weather,
			cfg.Location,
			effectiveTimezone,
			realClock,
			sched,
			fetchFn,
			pushOverlayToAll,
			publishNotification,
			registry.ConnectedIDs,
		)
		weatherCtrl.Start()
	}

	// Steps 15–16: Register signal handling and start the broker.
	serveErrCh := make(chan error, 1)

	go func() {
		serveErrCh <- broker.Serve(ctx, server)
	}()

	logger.L().Info("awtrix-controller started",
		"version", version,
		"commit", commit,
		"date", date,
		"mqttPort", cfg.MQTT.Port,
	)

	// Step 16: Block until a shutdown signal is received.
	<-ctx.Done()

	logger.L().Info("shutdown signal received")

	// Step 17: Shutdown sequence.
	sched.Stop()

	serveErr := <-serveErrCh
	if serveErr != nil {
		logger.L().Error("broker serve error during shutdown", "err", serveErr)
	}

	logger.L().Info("shutdown complete")

	return 0
}

// effectiveLevel returns the slog.Level derived from the CLI flag value and the
// env-var value. envVal is honored only when the CLI flag was not explicitly
// set by the user.
func effectiveLevel(flagVal, envVal string, flagWasSet bool) slog.Level {
	if flagWasSet || envVal == "" {
		return resolveLevel(flagVal)
	}

	return resolveLevel(envVal)
}

// resolveLevel parses a level string (e.g. "debug", "warn") into a slog.Level.
// Falls back to LevelInfo on empty or unrecognized input.
func resolveLevel(levelStr string) slog.Level {
	var level slog.Level

	unmarshalErr := level.UnmarshalText([]byte(levelStr))
	if unmarshalErr != nil {
		return slog.LevelInfo
	}

	return level
}
