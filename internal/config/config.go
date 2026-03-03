/*
 * MIT License
 *
 * Copyright (c) 2025 Roberto Leinardi
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
	MQTT         MQTTConfig         `yaml:"mqtt"`
	Location     LocationConfig     `yaml:"location"`
	Timezone     string             `yaml:"timezone"`
	EnergySaving EnergySavingConfig `yaml:"energy_saving"`
	Theme        ThemeConfig        `yaml:"theme"`
	Birthdays    []BirthdayConfig   `yaml:"birthdays"`
	NewYear      NewYearConfig      `yaml:"new_year"`
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
	// Elevation is optional; defaults to 0.0 (sea level).
	Elevation float64 `yaml:"elevation"`
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

// BirthdayConfig holds configuration for a single birthday alarm entry.
// After a successful Load, Rainbow and Wakeup are always non-nil.
type BirthdayConfig struct {
	// DateOfBirth must be in ISO-8601 format (YYYY-MM-DD). Entries with invalid
	// dates are skipped with a warning rather than causing a fatal error.
	DateOfBirth string `yaml:"date_of_birth"`
	Name        string `yaml:"name"`
	Duration    int    `yaml:"duration"`
	Icon        string `yaml:"icon"`
	// Rainbow enables the rainbow text effect. Nil is treated as true.
	Rainbow     *bool `yaml:"rainbow"`
	ScrollSpeed int   `yaml:"scroll_speed"`
	// Wakeup wakes the display from sleep. Nil is treated as true.
	Wakeup  *bool  `yaml:"wakeup"`
	RTTTL   string `yaml:"rtttl"`
	Message string `yaml:"message"`
}

// NewYearConfig holds configuration for the New Year notification.
// After a successful Load, Enabled, Rainbow, and Wakeup are always non-nil.
type NewYearConfig struct {
	// Enabled controls whether the notification fires. Nil is treated as true.
	Enabled  *bool  `yaml:"enabled"`
	Icon     string `yaml:"icon"`
	Duration int    `yaml:"duration"`
	// Rainbow enables the rainbow text effect. Nil is treated as true.
	Rainbow     *bool `yaml:"rainbow"`
	ScrollSpeed int   `yaml:"scroll_speed"`
	// Wakeup wakes the display from sleep. Nil is treated as true.
	Wakeup  *bool  `yaml:"wakeup"`
	Message string `yaml:"message"`
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

	cfg.Birthdays = filterAndDefaultBirthdays(cfg.Birthdays)
	applyNewYearDefaults(&cfg.NewYear)

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

func filterAndDefaultBirthdays(entries []BirthdayConfig) []BirthdayConfig {
	valid := make([]BirthdayConfig, 0, len(entries))

	for _, entry := range entries {
		_, parseErr := time.Parse("2006-01-02", entry.DateOfBirth)
		if parseErr != nil {
			logger.L().Warn("skipping birthday entry: invalid date_of_birth",
				"name", entry.Name,
				"date_of_birth", entry.DateOfBirth,
				"err", parseErr,
			)

			continue
		}

		applyBirthdayDefaults(&entry)
		valid = append(valid, entry)
	}

	return valid
}

func applyBirthdayDefaults(birthday *BirthdayConfig) {
	if birthday.Duration == 0 {
		birthday.Duration = DefaultBirthdayDuration
	}

	if birthday.Icon == "" {
		birthday.Icon = DefaultBirthdayIcon
	}

	if birthday.Rainbow == nil {
		birthday.Rainbow = boolPtr(true)
	}

	if birthday.ScrollSpeed == 0 {
		birthday.ScrollSpeed = DefaultBirthdayScrollSpeed
	}

	if birthday.Wakeup == nil {
		birthday.Wakeup = boolPtr(true)
	}
}

func applyNewYearDefaults(newYear *NewYearConfig) {
	if newYear.Enabled == nil {
		newYear.Enabled = boolPtr(true)
	}

	if newYear.Icon == "" {
		newYear.Icon = DefaultNewYearIcon
	}

	if newYear.Duration == 0 {
		newYear.Duration = DefaultNewYearDuration
	}

	if newYear.Rainbow == nil {
		newYear.Rainbow = boolPtr(true)
	}

	if newYear.ScrollSpeed == 0 {
		newYear.ScrollSpeed = DefaultNewYearScrollSpeed
	}

	if newYear.Wakeup == nil {
		newYear.Wakeup = boolPtr(true)
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
