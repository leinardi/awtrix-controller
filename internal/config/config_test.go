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

package config_test

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/config"
)

func boolPtr(b bool) *bool { return &b }

func TestNewConfigDebugView(t *testing.T) {
	t.Parallel()

	lat := 52.52
	lon := 13.405

	cfg := &config.Config{
		MQTT:         config.MQTTConfig{Port: 1883, Username: "user", Password: "secret"},
		Location:     config.LocationConfig{Latitude: &lat, Longitude: &lon},
		Timezone:     "Europe/Berlin",
		EnergySaving: config.EnergySavingConfig{Start: "00:30", End: "06:00"},
		Theme: config.ThemeConfig{
			Day:   config.ThemeColors{CalendarAccent: "#FF0000", Content: "#FFFFFF"},
			Night: config.ThemeColors{CalendarAccent: "#FF0000", Content: "#474747"},
		},
		ScheduledNotifications: make([]config.ScheduledNotificationConfig, 2),
		Weather: config.WeatherConfig{
			Enabled:                   true,
			PollIntervalMinutes:       15,
			OverlayHorizonMinutes:     60,
			NotificationHorizonHours:  8,
			NotificationRepeatMinutes: 60,
			InactiveAfterMissingPolls: 2,
			NotificationTextRepeat:    3,
			GustWarnKmh:               45.0,
			GustSevereKmh:             60.0,
			HeavyRainMmPer15Min:       5.0,
			FogVisibilityWarnM:        1000.0,
			FogVisibilitySevereM:      200.0,
			FrostTempC:                2.0,
			FrostDewPointDeltaC:       2.0,
			NotifyThunderstorm:        boolPtr(true),
			NotifyFreezingPrecip:      boolPtr(true),
			NotifyFrostRisk:           boolPtr(true),
			NotifyHeavyRain:           boolPtr(true),
			NotifyStrongGusts:         boolPtr(true),
			NotifySnow:                boolPtr(true),
			NotifyFog:                 boolPtr(false),
		},
	}

	view := config.NewConfigDebugView(cfg)

	if view.MQTTPort != 1883 {
		t.Errorf("MQTTPort = %d, want 1883", view.MQTTPort)
	}

	if view.MQTTUsername != "user" {
		t.Errorf("MQTTUsername = %q, want %q", view.MQTTUsername, "user")
	}

	if view.Latitude != lat {
		t.Errorf("Latitude = %v, want %v", view.Latitude, lat)
	}

	if view.Longitude != lon {
		t.Errorf("Longitude = %v, want %v", view.Longitude, lon)
	}

	if view.Timezone != "Europe/Berlin" {
		t.Errorf("Timezone = %q, want %q", view.Timezone, "Europe/Berlin")
	}

	if view.ScheduledNotificationsCount != 2 {
		t.Errorf("ScheduledNotificationsCount = %d, want 2", view.ScheduledNotificationsCount)
	}

	if !view.WeatherEnabled {
		t.Error("WeatherEnabled should be true")
	}

	if view.WeatherNotifyFog {
		t.Error("WeatherNotifyFog should be false")
	}

	if !view.WeatherNotifyThunderstorm {
		t.Error("WeatherNotifyThunderstorm should be true")
	}
}

// writeConfigFile writes content to a temporary file and returns its path.
func writeConfigFile(t *testing.T, content string) string {
	t.Helper()

	path := filepath.Join(t.TempDir(), "config.yaml")

	writeErr := os.WriteFile(path, []byte(content), 0o600)
	if writeErr != nil {
		t.Fatalf("write config file: %v", writeErr)
	}

	return path
}

// minimalValid is the smallest valid YAML config (required fields only).
const minimalValid = `
mqtt:
  username: "awtrix"
  password: "changeme"
location:
  latitude: 48.137154
  longitude: 11.576124
`

// TestLoadFileErrors covers file-system-level errors.
func TestLoadFileErrors(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		path    string
		wantErr string
	}{
		{
			name:    "file not found returns wrapped OS error",
			path:    "/nonexistent/path/to/config.yaml",
			wantErr: "read config file",
		},
		{
			name:    "invalid YAML returns parse error",
			path:    writeConfigFile(&testing.T{}, "mqtt:\n  username: [unclosed\n"),
			wantErr: "parse config",
		},
	}

	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			_, err := config.Load(testCase.path)
			if err == nil {
				t.Fatalf("Load() returned nil error, want error containing %q", testCase.wantErr)
			}

			if !strings.Contains(err.Error(), testCase.wantErr) {
				t.Fatalf("Load() error = %q, want it to contain %q", err.Error(), testCase.wantErr)
			}
		})
	}
}

// TestLoadRequiredFields verifies that missing required fields return the expected sentinel errors.
func TestLoadRequiredFields(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		content string
		wantErr error
	}{
		{
			name: "missing mqtt username",
			content: `
mqtt:
  password: "changeme"
location:
  latitude: 48.0
  longitude: 11.0
`,
			wantErr: config.ErrMQTTUsernameRequired,
		},
		{
			name: "missing mqtt password",
			content: `
mqtt:
  username: "awtrix"
location:
  latitude: 48.0
  longitude: 11.0
`,
			wantErr: config.ErrMQTTPasswordRequired,
		},
		{
			name: "missing location latitude",
			content: `
mqtt:
  username: "awtrix"
  password: "changeme"
location:
  longitude: 11.576124
`,
			wantErr: config.ErrLocationLatRequired,
		},
		{
			name: "missing location longitude",
			content: `
mqtt:
  username: "awtrix"
  password: "changeme"
location:
  latitude: 48.137154
`,
			wantErr: config.ErrLocationLonRequired,
		},
	}

	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			_, err := config.Load(writeConfigFile(t, testCase.content))
			if !errors.Is(err, testCase.wantErr) {
				t.Fatalf("Load() error = %v, want %v", err, testCase.wantErr)
			}
		})
	}
}

// TestLoadTimezone covers timezone validation and the absent-timezone path.
func TestLoadTimezone(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		content string
		wantErr string
		wantTZ  string
	}{
		{
			name:    "absent timezone is accepted with empty string",
			content: minimalValid,
			wantTZ:  "",
		},
		{
			name: "valid IANA timezone is accepted",
			content: minimalValid + `timezone: "Europe/Berlin"
`,
			wantTZ: "Europe/Berlin",
		},
		{
			name: "invalid IANA timezone returns error",
			content: minimalValid + `timezone: "Not/AReal/Timezone"
`,
			wantErr: "invalid timezone",
		},
	}

	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			cfg, err := config.Load(writeConfigFile(t, testCase.content))

			if testCase.wantErr != "" {
				if err == nil || !strings.Contains(err.Error(), testCase.wantErr) {
					t.Fatalf("Load() error = %v, want it to contain %q", err, testCase.wantErr)
				}

				return
			}

			if err != nil {
				t.Fatalf("Load() unexpected error: %v", err)
			}

			if cfg.Timezone != testCase.wantTZ {
				t.Errorf("Timezone = %q, want %q", cfg.Timezone, testCase.wantTZ)
			}
		})
	}
}

// TestLoadThemeColors verifies color validation and default application.
func TestLoadThemeColors(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name    string
		content string
		wantErr error
	}{
		{
			name: "invalid hex color returns ErrInvalidHexColor",
			content: minimalValid + `theme:
  day:
    calendar_accent: "red"
`,
			wantErr: config.ErrInvalidHexColor,
		},
	}

	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			_, err := config.Load(writeConfigFile(t, testCase.content))
			if !errors.Is(err, testCase.wantErr) {
				t.Fatalf("Load() error = %v, want %v", err, testCase.wantErr)
			}
		})
	}
}

// TestLoadDefaults verifies that the minimal valid config receives all expected defaults.
func TestLoadDefaults(t *testing.T) {
	t.Parallel()

	cfg, err := config.Load(writeConfigFile(t, minimalValid))
	if err != nil {
		t.Fatalf("Load() unexpected error: %v", err)
	}

	if cfg.MQTT.Port != config.DefaultMQTTPort {
		t.Errorf("MQTT.Port = %d, want %d", cfg.MQTT.Port, config.DefaultMQTTPort)
	}

	if cfg.EnergySaving.Start != config.DefaultEnergySavingStart {
		t.Errorf(
			"EnergySaving.Start = %q, want %q",
			cfg.EnergySaving.Start,
			config.DefaultEnergySavingStart,
		)
	}

	if cfg.EnergySaving.End != config.DefaultEnergySavingEnd {
		t.Errorf(
			"EnergySaving.End = %q, want %q",
			cfg.EnergySaving.End,
			config.DefaultEnergySavingEnd,
		)
	}

	if cfg.Theme.Day.CalendarAccent != config.DefaultThemeDayAccent {
		t.Errorf(
			"Theme.Day.CalendarAccent = %q, want %q",
			cfg.Theme.Day.CalendarAccent,
			config.DefaultThemeDayAccent,
		)
	}

	if cfg.Theme.Day.Content != config.DefaultThemeDayContent {
		t.Errorf(
			"Theme.Day.Content = %q, want %q",
			cfg.Theme.Day.Content,
			config.DefaultThemeDayContent,
		)
	}

	if cfg.Theme.Night.Content != config.DefaultThemeNightContent {
		t.Errorf(
			"Theme.Night.Content = %q, want %q",
			cfg.Theme.Night.Content,
			config.DefaultThemeNightContent,
		)
	}

	if len(cfg.ScheduledNotifications) != 0 {
		t.Errorf("ScheduledNotifications = %v, want empty", cfg.ScheduledNotifications)
	}
}

// TestLoadScheduledNotificationEnabled verifies that enabled defaults to true
// when absent and that enabled: false is preserved.
func TestLoadScheduledNotificationEnabled(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name        string
		content     string
		wantEnabled bool
	}{
		{
			name: "enabled absent defaults to true",
			content: minimalValid + `scheduled_notifications:
  - name: "New Year"
    repeat: yearly
    date: "01-01"
`,
			wantEnabled: true,
		},
		{
			name: "enabled false is preserved",
			content: minimalValid + `scheduled_notifications:
  - name: "New Year"
    repeat: yearly
    date: "01-01"
    enabled: false
`,
			wantEnabled: false,
		},
	}

	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			cfg, err := config.Load(writeConfigFile(t, testCase.content))
			if err != nil {
				t.Fatalf("Load() unexpected error: %v", err)
			}

			if len(cfg.ScheduledNotifications) != 1 {
				t.Fatalf(
					"len(ScheduledNotifications) = %d, want 1",
					len(cfg.ScheduledNotifications),
				)
			}

			entry := cfg.ScheduledNotifications[0]
			if entry.Enabled == nil || *entry.Enabled != testCase.wantEnabled {
				t.Errorf("Enabled = %v, want %v", entry.Enabled, testCase.wantEnabled)
			}
		})
	}
}

// TestLoadScheduledNotificationsWeekly covers weekday slice validation for weekly entries.
func TestLoadScheduledNotificationsWeekly(t *testing.T) {
	t.Parallel()

	t.Run("empty weekdays list is skipped", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "Standup"
    repeat: weekly
    weekdays: []
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 0 {
			t.Errorf(
				"len(ScheduledNotifications) = %d, want 0 (empty weekdays)",
				len(cfg.ScheduledNotifications),
			)
		}
	})

	t.Run("invalid weekday in list is skipped", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "Bad"
    repeat: weekly
    weekdays:
      - monday
      - funday
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 0 {
			t.Errorf(
				"len(ScheduledNotifications) = %d, want 0 (invalid weekday)",
				len(cfg.ScheduledNotifications),
			)
		}
	})

	t.Run("valid weekdays list is accepted", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "Standup"
    repeat: weekly
    weekdays:
      - monday
      - tuesday
      - wednesday
      - thursday
      - friday
    time: "09:45"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 1 {
			t.Fatalf("len(ScheduledNotifications) = %d, want 1", len(cfg.ScheduledNotifications))
		}

		entry := cfg.ScheduledNotifications[0]

		wantWeekdays := []string{"monday", "tuesday", "wednesday", "thursday", "friday"}
		if len(entry.Weekdays) != len(wantWeekdays) {
			t.Fatalf("len(Weekdays) = %d, want %d", len(entry.Weekdays), len(wantWeekdays))
		}

		for index, want := range wantWeekdays {
			if entry.Weekdays[index] != want {
				t.Errorf("Weekdays[%d] = %q, want %q", index, entry.Weekdays[index], want)
			}
		}
	})
}

// TestWeatherConfigDefaultsApplied verifies that an empty WeatherConfig receives all expected defaults.
func TestWeatherConfigDefaultsApplied(t *testing.T) {
	t.Parallel()

	cfg, err := config.Load(writeConfigFile(t, minimalValid))
	if err != nil {
		t.Fatalf("Load() unexpected error: %v", err)
	}

	weather := cfg.Weather

	if weather.PollIntervalMinutes != config.DefaultWeatherPollIntervalMinutes {
		t.Errorf(
			"PollIntervalMinutes = %d, want %d",
			weather.PollIntervalMinutes,
			config.DefaultWeatherPollIntervalMinutes,
		)
	}

	if weather.OverlayHorizonMinutes != config.DefaultWeatherOverlayHorizonMinutes {
		t.Errorf(
			"OverlayHorizonMinutes = %d, want %d",
			weather.OverlayHorizonMinutes,
			config.DefaultWeatherOverlayHorizonMinutes,
		)
	}

	if weather.NotificationHorizonHours != config.DefaultWeatherNotificationHorizonHours {
		t.Errorf(
			"NotificationHorizonHours = %d, want %d",
			weather.NotificationHorizonHours,
			config.DefaultWeatherNotificationHorizonHours,
		)
	}

	if weather.NotificationRepeatMinutes != config.DefaultWeatherNotificationRepeatMinutes {
		t.Errorf(
			"NotificationRepeatMinutes = %d, want %d",
			weather.NotificationRepeatMinutes,
			config.DefaultWeatherNotificationRepeatMinutes,
		)
	}

	if weather.GustWarnKmh != config.DefaultWeatherGustWarnKmh {
		t.Errorf("GustWarnKmh = %v, want %v", weather.GustWarnKmh, config.DefaultWeatherGustWarnKmh)
	}

	if weather.GustSevereKmh != config.DefaultWeatherGustSevereKmh {
		t.Errorf(
			"GustSevereKmh = %v, want %v",
			weather.GustSevereKmh,
			config.DefaultWeatherGustSevereKmh,
		)
	}

	if weather.HeavyRainMmPer15Min != config.DefaultWeatherHeavyRainMmPer15Min {
		t.Errorf(
			"HeavyRainMmPer15Min = %v, want %v",
			weather.HeavyRainMmPer15Min,
			config.DefaultWeatherHeavyRainMmPer15Min,
		)
	}

	if weather.FrostTempC != config.DefaultWeatherFrostTempC {
		t.Errorf("FrostTempC = %v, want %v", weather.FrostTempC, config.DefaultWeatherFrostTempC)
	}
}

// TestWeatherConfigUserValuesPreserved verifies that explicit values are not overwritten.
func TestWeatherConfigUserValuesPreserved(t *testing.T) {
	t.Parallel()

	content := minimalValid + `weather:
  enabled: true
  poll_interval_minutes: 30
  gust_warn_kmh: 55.0
`

	cfg, err := config.Load(writeConfigFile(t, content))
	if err != nil {
		t.Fatalf("Load() unexpected error: %v", err)
	}

	if cfg.Weather.PollIntervalMinutes != 30 {
		t.Errorf("PollIntervalMinutes = %d, want 30", cfg.Weather.PollIntervalMinutes)
	}

	if cfg.Weather.GustWarnKmh != 55.0 {
		t.Errorf("GustWarnKmh = %v, want 55.0", cfg.Weather.GustWarnKmh)
	}
}

// TestWeatherConfigFogDefaultsFalse verifies that NotifyFog defaults to false.
func TestWeatherConfigFogDefaultsFalse(t *testing.T) {
	t.Parallel()

	cfg, err := config.Load(writeConfigFile(t, minimalValid))
	if err != nil {
		t.Fatalf("Load() unexpected error: %v", err)
	}

	if cfg.Weather.NotifyFog == nil || *cfg.Weather.NotifyFog {
		t.Error("NotifyFog should default to false")
	}
}

// TestWeatherConfigNotifyDefaultsTrue verifies that thunderstorm/snow/etc. default to true.
func TestWeatherConfigNotifyDefaultsTrue(t *testing.T) {
	t.Parallel()

	cfg, err := config.Load(writeConfigFile(t, minimalValid))
	if err != nil {
		t.Fatalf("Load() unexpected error: %v", err)
	}

	weather := cfg.Weather

	notifiers := map[string]*bool{
		"NotifyThunderstorm":   weather.NotifyThunderstorm,
		"NotifyFreezingPrecip": weather.NotifyFreezingPrecip,
		"NotifyFrostRisk":      weather.NotifyFrostRisk,
		"NotifyHeavyRain":      weather.NotifyHeavyRain,
		"NotifyStrongGusts":    weather.NotifyStrongGusts,
		"NotifySnow":           weather.NotifySnow,
	}

	for name, ptr := range notifiers {
		if ptr == nil || !*ptr {
			t.Errorf("%s should default to true", name)
		}
	}
}

// TestLoadScheduledNotifications covers filtering (bad repeat/date skipped)
// and default application.
//
//nolint:gocyclo,cyclop // multiple sub-tests each with several assertions
func TestLoadScheduledNotifications(t *testing.T) {
	t.Parallel()

	t.Run("bad repeat is skipped, valid entry is kept", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "Bad"
    repeat: "invalid"
  - name: "Alice"
    repeat: yearly
    date: "04-16"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 1 {
			t.Fatalf("len(ScheduledNotifications) = %d, want 1", len(cfg.ScheduledNotifications))
		}

		if cfg.ScheduledNotifications[0].Name != "Alice" {
			t.Errorf(
				"ScheduledNotifications[0].Name = %q, want \"Alice\"",
				cfg.ScheduledNotifications[0].Name,
			)
		}
	})

	t.Run("bad date is skipped for yearly", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "BadDate"
    repeat: yearly
    date: "not-a-date"
  - name: "Alice"
    repeat: yearly
    date: "04-16"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 1 {
			t.Fatalf("len(ScheduledNotifications) = %d, want 1", len(cfg.ScheduledNotifications))
		}

		if cfg.ScheduledNotifications[0].Name != "Alice" {
			t.Errorf(
				"ScheduledNotifications[0].Name = %q, want \"Alice\"",
				cfg.ScheduledNotifications[0].Name,
			)
		}
	})

	t.Run("defaults applied to yearly entry", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `scheduled_notifications:
  - name: "Alice"
    repeat: yearly
    date: "04-16"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.ScheduledNotifications) != 1 {
			t.Fatalf("len(ScheduledNotifications) = %d, want 1", len(cfg.ScheduledNotifications))
		}

		entry := cfg.ScheduledNotifications[0]

		if entry.Duration != config.DefaultScheduledNotificationDuration {
			t.Errorf(
				"Duration = %d, want %d",
				entry.Duration,
				config.DefaultScheduledNotificationDuration,
			)
		}

		if entry.Icon != config.DefaultScheduledNotificationIcon {
			t.Errorf("Icon = %q, want %q", entry.Icon, config.DefaultScheduledNotificationIcon)
		}

		if entry.Rainbow == nil || *entry.Rainbow {
			t.Error("Rainbow should default to false")
		}

		if entry.Wakeup == nil || !*entry.Wakeup {
			t.Error("Wakeup should default to true")
		}

		if entry.ScrollSpeed != config.DefaultScheduledNotificationScrollSpeed {
			t.Errorf(
				"ScrollSpeed = %d, want %d",
				entry.ScrollSpeed,
				config.DefaultScheduledNotificationScrollSpeed,
			)
		}

		if entry.Time != config.DefaultScheduledNotificationTime {
			t.Errorf("Time = %q, want %q", entry.Time, config.DefaultScheduledNotificationTime)
		}
	})
}
