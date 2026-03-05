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

// Package config loads and validates the application YAML configuration file.
// All required fields are checked; optional fields receive documented defaults.
// The configuration is loaded once at startup and never hot-reloaded.
package config

import (
	"errors"
	"fmt"
	"os"
	"time"

	"github.com/leinardi/awtrix-controller/internal/logger"
	"gopkg.in/yaml.v3"
)

// Sentinel errors returned by Validate for individual field constraint violations.
// Callers may test for these with errors.Is.
var (
	// ErrMQTTUsernameRequired is returned when mqtt.username is absent.
	ErrMQTTUsernameRequired = errors.New("mqtt.username is required")

	// ErrMQTTPasswordRequired is returned when mqtt.password is absent.
	ErrMQTTPasswordRequired = errors.New("mqtt.password is required")

	// ErrLocationLatRequired is returned when location.latitude is absent.
	ErrLocationLatRequired = errors.New("location.latitude is required")

	// ErrLocationLonRequired is returned when location.longitude is absent.
	ErrLocationLonRequired = errors.New("location.longitude is required")

	// ErrInvalidHexColor is returned when a color field is not a valid #RRGGBB string.
	ErrInvalidHexColor = errors.New("not a valid #RRGGBB color")
)

// Config is the top-level application configuration.
type Config struct {
	MQTT                   MQTTConfig                    `yaml:"mqtt"`
	Location               LocationConfig                `yaml:"location"`
	Timezone               string                        `yaml:"timezone"`
	EnergySaving           EnergySavingConfig            `yaml:"energy_saving"`
	Theme                  ThemeConfig                   `yaml:"theme"`
	ScheduledNotifications []ScheduledNotificationConfig `yaml:"scheduled_notifications"`
	Weather                WeatherConfig                 `yaml:"weather"`
}

// WeatherConfig holds configuration for weather overlay and warning notifications.
// Enabled=false (the default) disables all weather features.
type WeatherConfig struct {
	Enabled                   bool    `yaml:"enabled"`
	PollIntervalMinutes       int     `yaml:"poll_interval_minutes"`
	OverlayHorizonMinutes     int     `yaml:"overlay_horizon_minutes"`
	NotificationHorizonHours  int     `yaml:"notification_horizon_hours"`
	NotificationRepeatMinutes int     `yaml:"notification_repeat_minutes"`
	InactiveAfterMissingPolls int     `yaml:"inactive_after_missing_polls"`
	DataStaleTTLMinutes       int     `yaml:"data_stale_ttl_minutes"`
	NotificationTextRepeat    int     `yaml:"notification_text_repeat"`
	GustWarnKmh               float64 `yaml:"gust_warn_kmh"`
	GustSevereKmh             float64 `yaml:"gust_severe_kmh"`
	HeavyRainMmPer15Min       float64 `yaml:"heavy_rain_mm_per_15min"`
	FogVisibilityWarnM        float64 `yaml:"fog_visibility_warn_m"`
	FogVisibilitySevereM      float64 `yaml:"fog_visibility_severe_m"`
	FrostTempC                float64 `yaml:"frost_temp_c"`
	FrostDewPointDeltaC       float64 `yaml:"frost_dew_point_delta_c"`
	FrostWarnPrecipWindowH    float64 `yaml:"frost_warn_precip_window_h"`
	FrostWarnPrecipMm         float64 `yaml:"frost_warn_precip_mm"`
	// *bool: nil → apply default; explicit false → disable
	NotifyThunderstorm   *bool `yaml:"notify_thunderstorm"`
	NotifyFreezingPrecip *bool `yaml:"notify_freezing_precip"`
	NotifyFrostRisk      *bool `yaml:"notify_frost_risk"`
	NotifyHeavyRain      *bool `yaml:"notify_heavy_rain"`
	NotifyStrongGusts    *bool `yaml:"notify_strong_gusts"`
	NotifySnow           *bool `yaml:"notify_snow"`
	NotifyFog            *bool `yaml:"notify_fog"` // default false (noisier)
}

// MQTTConfig holds MQTT broker settings.
type MQTTConfig struct {
	// Port is the MQTT TCP listen port. Defaults to DefaultMQTTPort when absent.
	Port int `yaml:"port"`
	// WSPort is the WebSocket listen port. Nil means WebSocket is disabled.
	WSPort   *int   `yaml:"ws_port"`
	Username string `yaml:"username"`
	Password string `yaml:"password"`
}

// LocationConfig holds the geographic coordinates used for sunrise/sunset
// calculation. After a successful Load, Latitude and Longitude are always
// non-nil.
type LocationConfig struct {
	// Latitude is required; pointer so that absence (nil) is detectable.
	Latitude *float64 `yaml:"latitude"`
	// Longitude is required; pointer so that absence (nil) is detectable.
	Longitude *float64 `yaml:"longitude"`
}

// EnergySavingConfig holds the energy-saving window configuration.
// Both times are in HH:MM format within the configured timezone.
// The window may span midnight (e.g. Start="23:00", End="05:00").
type EnergySavingConfig struct {
	Start string `yaml:"start"`
	End   string `yaml:"end"`
}

// ThemeConfig holds the color sets for day and night operation modes.
type ThemeConfig struct {
	Day   ThemeColors `yaml:"day"`
	Night ThemeColors `yaml:"night"`
}

// ThemeColors holds the two managed colors for a single day/night theme.
type ThemeColors struct {
	// CalendarAccent is the calendar header and active-weekday color (#RRGGBB).
	CalendarAccent string `yaml:"calendar_accent"`
	// Content is the text, date, and inactive-weekday color (#RRGGBB).
	Content string `yaml:"content"`
}

// ScheduledNotificationConfig holds configuration for a single recurring
// scheduled notification. After a successful Load, Enabled, Rainbow, and
// Wakeup are always non-nil; Time is always a valid HH:MM string.
type ScheduledNotificationConfig struct {
	Name     string   `yaml:"name"`
	Message  string   `yaml:"message"`  // template: {name}, {year}
	Repeat   string   `yaml:"repeat"`   // daily | weekly | monthly | yearly
	Date     string   `yaml:"date"`     // MM-DD, required for yearly
	Day      int      `yaml:"day"`      // 1–31, required for monthly
	Weekdays []string `yaml:"weekdays"` // monday–sunday list, required for weekly
	Time     string   `yaml:"time"`     // HH:MM, default "00:00"
	// Enabled controls whether the notification fires. Nil is treated as true.
	Enabled     *bool  `yaml:"enabled"`
	Duration    int    `yaml:"duration"`     // default: 60
	Icon        string `yaml:"icon"`         // default: "9597"
	Rainbow     *bool  `yaml:"rainbow"`      // default: false
	ScrollSpeed int    `yaml:"scroll_speed"` // default: 50
	// Wakeup wakes the display from sleep. Nil is treated as true.
	Wakeup *bool  `yaml:"wakeup"`
	RTTTL  string `yaml:"rtttl"` // optional RTTTL melody string
}

// Load reads the YAML configuration file at path, unmarshals it, applies
// defaults to optional fields, and validates all required fields and value
// constraints. It returns an error wrapping the OS error if the file is absent
// or unreadable, a parse error (with line details) if the YAML is malformed,
// or a validation error describing the first constraint violation encountered.
func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read config file: %w", err)
	}

	var cfg Config

	unmarshalErr := yaml.Unmarshal(data, &cfg)
	if unmarshalErr != nil {
		return nil, fmt.Errorf("parse config: %w", unmarshalErr)
	}

	validateErr := Validate(&cfg)
	if validateErr != nil {
		return nil, validateErr
	}

	return &cfg, nil
}

// Validate checks required fields, applies defaults to optional fields, and
// validates value constraints. It is exported so that callers can validate
// in-memory configs without touching the filesystem. Birthday entries with an
// invalid date_of_birth are skipped with a Warn log rather than returning an
// error.
func Validate(cfg *Config) error {
	var err error

	err = validateMQTT(&cfg.MQTT)
	if err != nil {
		return err
	}

	err = validateLocation(&cfg.Location)
	if err != nil {
		return err
	}

	err = validateTimezone(cfg.Timezone)
	if err != nil {
		return err
	}

	applyEnergySavingDefaults(&cfg.EnergySaving)

	err = validateTheme(&cfg.Theme)
	if err != nil {
		return err
	}

	cfg.ScheduledNotifications = filterAndDefaultScheduledNotifications(cfg.ScheduledNotifications)

	applyWeatherDefaults(&cfg.Weather)

	return nil
}

// --- validation helpers ---

func validateMQTT(mqtt *MQTTConfig) error {
	if mqtt.Username == "" {
		return ErrMQTTUsernameRequired
	}

	if mqtt.Password == "" {
		return ErrMQTTPasswordRequired
	}

	if mqtt.Port == 0 {
		mqtt.Port = DefaultMQTTPort
	}

	return nil
}

func validateLocation(loc *LocationConfig) error {
	if loc.Latitude == nil {
		return ErrLocationLatRequired
	}

	if loc.Longitude == nil {
		return ErrLocationLonRequired
	}

	return nil
}

func validateTimezone(timezone string) error {
	if timezone == "" {
		return nil // caller will warn and fall back to the system timezone
	}

	_, locErr := time.LoadLocation(timezone)
	if locErr != nil {
		return fmt.Errorf("invalid timezone %q: %w", timezone, locErr)
	}

	return nil
}

func applyEnergySavingDefaults(energySaving *EnergySavingConfig) {
	if energySaving.Start == "" {
		energySaving.Start = DefaultEnergySavingStart
	}

	if energySaving.End == "" {
		energySaving.End = DefaultEnergySavingEnd
	}
}

func validateTheme(theme *ThemeConfig) error {
	var err error

	err = validateAndDefaultColors(&theme.Day, DefaultThemeDayAccent, DefaultThemeDayContent)
	if err != nil {
		return fmt.Errorf("theme.day: %w", err)
	}

	err = validateAndDefaultColors(&theme.Night, DefaultThemeNightAccent, DefaultThemeNightContent)
	if err != nil {
		return fmt.Errorf("theme.night: %w", err)
	}

	return nil
}

func validateAndDefaultColors(colors *ThemeColors, defaultAccent, defaultContent string) error {
	if colors.CalendarAccent == "" {
		colors.CalendarAccent = defaultAccent
	} else if !isValidHexColor(colors.CalendarAccent) {
		return fmt.Errorf("calendar_accent %q: %w", colors.CalendarAccent, ErrInvalidHexColor)
	}

	if colors.Content == "" {
		colors.Content = defaultContent
	} else if !isValidHexColor(colors.Content) {
		return fmt.Errorf("content %q: %w", colors.Content, ErrInvalidHexColor)
	}

	return nil
}

func filterAndDefaultScheduledNotifications(
	entries []ScheduledNotificationConfig,
) []ScheduledNotificationConfig {
	valid := make([]ScheduledNotificationConfig, 0, len(entries))

	for entryIdx := range entries {
		if !validateScheduledNotificationEntry(&entries[entryIdx]) {
			continue
		}

		applyScheduledNotificationDefaults(&entries[entryIdx])
		valid = append(valid, entries[entryIdx])
	}

	return valid
}

func validateScheduledNotificationEntry(entry *ScheduledNotificationConfig) bool {
	switch entry.Repeat {
	case "daily":
		// no extra required fields
	case "weekly":
		if len(entry.Weekdays) == 0 {
			logger.L().Warn("skipping scheduled_notification: weekdays list is empty",
				"name", entry.Name,
			)

			return false
		}

		for _, weekday := range entry.Weekdays {
			switch weekday {
			case "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday":
				// valid
			default:
				logger.L().Warn("skipping scheduled_notification: invalid weekday",
					"name", entry.Name,
					"weekday", weekday,
				)

				return false
			}
		}
	case "monthly":
		if entry.Day < 1 || entry.Day > 31 {
			logger.L().Warn("skipping scheduled_notification: day out of range",
				"name", entry.Name,
				"day", entry.Day,
			)

			return false
		}
	case "yearly":
		_, parseErr := time.Parse("01-02", entry.Date)
		if parseErr != nil {
			logger.L().Warn("skipping scheduled_notification: invalid date",
				"name", entry.Name,
				"date", entry.Date,
			)

			return false
		}
	default:
		logger.L().Warn("skipping scheduled_notification: unknown repeat",
			"name", entry.Name,
			"repeat", entry.Repeat,
		)

		return false
	}

	if entry.Time != "" {
		_, timeErr := time.Parse("15:04", entry.Time)
		if timeErr != nil {
			logger.L().Warn("skipping scheduled_notification: invalid time",
				"name", entry.Name,
				"time", entry.Time,
			)

			return false
		}
	}

	return true
}

func applyScheduledNotificationDefaults(entry *ScheduledNotificationConfig) {
	if entry.Time == "" {
		entry.Time = DefaultScheduledNotificationTime
	}

	if entry.Enabled == nil {
		entry.Enabled = boolPtr(true)
	}

	if entry.Duration == 0 {
		entry.Duration = DefaultScheduledNotificationDuration
	}

	if entry.Icon == "" {
		entry.Icon = DefaultScheduledNotificationIcon
	}

	if entry.Rainbow == nil {
		entry.Rainbow = boolPtr(false)
	}

	if entry.ScrollSpeed == 0 {
		entry.ScrollSpeed = DefaultScheduledNotificationScrollSpeed
	}

	if entry.Wakeup == nil {
		entry.Wakeup = boolPtr(true)
	}
}

//nolint:cyclop,gocyclo // setting 14 optional fields sequentially is unavoidably long; splitting would obscure intent
func applyWeatherDefaults(weather *WeatherConfig) {
	if weather.PollIntervalMinutes == 0 {
		weather.PollIntervalMinutes = DefaultWeatherPollIntervalMinutes
	}

	if weather.OverlayHorizonMinutes == 0 {
		weather.OverlayHorizonMinutes = DefaultWeatherOverlayHorizonMinutes
	}

	if weather.NotificationHorizonHours == 0 {
		weather.NotificationHorizonHours = DefaultWeatherNotificationHorizonHours
	}

	if weather.NotificationRepeatMinutes == 0 {
		weather.NotificationRepeatMinutes = DefaultWeatherNotificationRepeatMinutes
	}

	if weather.InactiveAfterMissingPolls == 0 {
		weather.InactiveAfterMissingPolls = DefaultWeatherInactiveAfterMissingPolls
	}

	if weather.DataStaleTTLMinutes == 0 {
		weather.DataStaleTTLMinutes = DefaultWeatherDataStaleTTLMinutes
	}

	if weather.NotificationTextRepeat == 0 {
		weather.NotificationTextRepeat = DefaultWeatherNotificationTextRepeat
	}

	if weather.GustWarnKmh == 0 {
		weather.GustWarnKmh = DefaultWeatherGustWarnKmh
	}

	if weather.GustSevereKmh == 0 {
		weather.GustSevereKmh = DefaultWeatherGustSevereKmh
	}

	if weather.HeavyRainMmPer15Min == 0 {
		weather.HeavyRainMmPer15Min = DefaultWeatherHeavyRainMmPer15Min
	}

	if weather.FogVisibilityWarnM == 0 {
		weather.FogVisibilityWarnM = DefaultWeatherFogVisibilityWarnM
	}

	if weather.FogVisibilitySevereM == 0 {
		weather.FogVisibilitySevereM = DefaultWeatherFogVisibilitySevereM
	}

	if weather.FrostTempC == 0 {
		weather.FrostTempC = DefaultWeatherFrostTempC
	}

	if weather.FrostDewPointDeltaC == 0 {
		weather.FrostDewPointDeltaC = DefaultWeatherFrostDewPointDeltaC
	}

	if weather.FrostWarnPrecipWindowH == 0 {
		weather.FrostWarnPrecipWindowH = DefaultWeatherFrostWarnPrecipWindowH
	}

	if weather.FrostWarnPrecipMm == 0 {
		weather.FrostWarnPrecipMm = DefaultWeatherFrostWarnPrecipMm
	}

	if weather.NotifyThunderstorm == nil {
		weather.NotifyThunderstorm = boolPtr(true)
	}

	if weather.NotifyFreezingPrecip == nil {
		weather.NotifyFreezingPrecip = boolPtr(true)
	}

	if weather.NotifyFrostRisk == nil {
		weather.NotifyFrostRisk = boolPtr(true)
	}

	if weather.NotifyHeavyRain == nil {
		weather.NotifyHeavyRain = boolPtr(true)
	}

	if weather.NotifyStrongGusts == nil {
		weather.NotifyStrongGusts = boolPtr(true)
	}

	if weather.NotifySnow == nil {
		weather.NotifySnow = boolPtr(true)
	}

	if weather.NotifyFog == nil {
		weather.NotifyFog = boolPtr(false)
	}
}

// ConfigDebugView is a flat, slog-friendly snapshot of Config used only for
// debug logging. All *bool pointer fields are dereferenced (safe after Validate).
// The MQTT password is intentionally omitted.
type ConfigDebugView struct {
	// MQTT
	MQTTPort     int
	MQTTUsername string
	// Location
	Latitude  float64
	Longitude float64
	// Timezone / energy-saving window
	Timezone          string
	EnergySavingStart string
	EnergySavingEnd   string
	// Theme
	ThemeDayAccent    string
	ThemeDayContent   string
	ThemeNightAccent  string
	ThemeNightContent string
	// Scheduled notifications
	ScheduledNotificationsCount int
	// Weather
	WeatherEnabled                   bool
	WeatherPollIntervalMinutes       int
	WeatherOverlayHorizonMinutes     int
	WeatherNotificationHorizonHours  int
	WeatherNotificationRepeatMinutes int
	WeatherInactiveAfterMissingPolls int
	WeatherNotificationTextRepeat    int
	WeatherGustWarnKmh               float64
	WeatherGustSevereKmh             float64
	WeatherHeavyRainMmPer15Min       float64
	WeatherFogVisibilityWarnM        float64
	WeatherFogVisibilitySevereM      float64
	WeatherFrostTempC                float64
	WeatherFrostDewPointDeltaC       float64
	WeatherFrostWarnPrecipWindowH    float64
	WeatherFrostWarnPrecipMm         float64
	WeatherNotifyThunderstorm        bool
	WeatherNotifyFreezingPrecip      bool
	WeatherNotifyFrostRisk           bool
	WeatherNotifyHeavyRain           bool
	WeatherNotifyStrongGusts         bool
	WeatherNotifySnow                bool
	WeatherNotifyFog                 bool
}

// NewConfigDebugView builds a ConfigDebugView from a validated Config.
// All pointer fields are safe to dereference because Validate always fills them.
func NewConfigDebugView(cfg *Config) ConfigDebugView {
	return ConfigDebugView{
		MQTTPort:                         cfg.MQTT.Port,
		MQTTUsername:                     cfg.MQTT.Username,
		Latitude:                         *cfg.Location.Latitude,
		Longitude:                        *cfg.Location.Longitude,
		Timezone:                         cfg.Timezone,
		EnergySavingStart:                cfg.EnergySaving.Start,
		EnergySavingEnd:                  cfg.EnergySaving.End,
		ThemeDayAccent:                   cfg.Theme.Day.CalendarAccent,
		ThemeDayContent:                  cfg.Theme.Day.Content,
		ThemeNightAccent:                 cfg.Theme.Night.CalendarAccent,
		ThemeNightContent:                cfg.Theme.Night.Content,
		ScheduledNotificationsCount:      len(cfg.ScheduledNotifications),
		WeatherEnabled:                   cfg.Weather.Enabled,
		WeatherPollIntervalMinutes:       cfg.Weather.PollIntervalMinutes,
		WeatherOverlayHorizonMinutes:     cfg.Weather.OverlayHorizonMinutes,
		WeatherNotificationHorizonHours:  cfg.Weather.NotificationHorizonHours,
		WeatherNotificationRepeatMinutes: cfg.Weather.NotificationRepeatMinutes,
		WeatherInactiveAfterMissingPolls: cfg.Weather.InactiveAfterMissingPolls,
		WeatherNotificationTextRepeat:    cfg.Weather.NotificationTextRepeat,
		WeatherGustWarnKmh:               cfg.Weather.GustWarnKmh,
		WeatherGustSevereKmh:             cfg.Weather.GustSevereKmh,
		WeatherHeavyRainMmPer15Min:       cfg.Weather.HeavyRainMmPer15Min,
		WeatherFogVisibilityWarnM:        cfg.Weather.FogVisibilityWarnM,
		WeatherFogVisibilitySevereM:      cfg.Weather.FogVisibilitySevereM,
		WeatherFrostTempC:                cfg.Weather.FrostTempC,
		WeatherFrostDewPointDeltaC:       cfg.Weather.FrostDewPointDeltaC,
		WeatherFrostWarnPrecipWindowH:    cfg.Weather.FrostWarnPrecipWindowH,
		WeatherFrostWarnPrecipMm:         cfg.Weather.FrostWarnPrecipMm,
		WeatherNotifyThunderstorm:        *cfg.Weather.NotifyThunderstorm,
		WeatherNotifyFreezingPrecip:      *cfg.Weather.NotifyFreezingPrecip,
		WeatherNotifyFrostRisk:           *cfg.Weather.NotifyFrostRisk,
		WeatherNotifyHeavyRain:           *cfg.Weather.NotifyHeavyRain,
		WeatherNotifyStrongGusts:         *cfg.Weather.NotifyStrongGusts,
		WeatherNotifySnow:                *cfg.Weather.NotifySnow,
		WeatherNotifyFog:                 *cfg.Weather.NotifyFog,
	}
}

// isValidHexColor reports whether s is a valid #RRGGBB color string.
func isValidHexColor(s string) bool {
	if len(s) != hexColorLen || s[0] != '#' {
		return false
	}

	for _, ch := range s[1:] {
		if (ch < '0' || ch > '9') && (ch < 'A' || ch > 'F') && (ch < 'a' || ch > 'f') {
			return false
		}
	}

	return true
}

// boolPtr returns a pointer to the given bool value.
func boolPtr(b bool) *bool { return &b }
