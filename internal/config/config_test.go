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

package config_test

import (
	"errors"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/config"
)

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

	if cfg.NewYear.Enabled == nil || !*cfg.NewYear.Enabled {
		t.Error("NewYear.Enabled should default to true")
	}

	if cfg.NewYear.Icon != config.DefaultNewYearIcon {
		t.Errorf("NewYear.Icon = %q, want %q", cfg.NewYear.Icon, config.DefaultNewYearIcon)
	}
}

// TestLoadNewYear verifies new_year defaults and the enabled=false override.
func TestLoadNewYear(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name        string
		content     string
		wantEnabled bool
	}{
		{
			name:        "new_year enabled absent defaults to true",
			content:     minimalValid + "new_year:\n  icon: \"5855\"\n",
			wantEnabled: true,
		},
		{
			name:        "new_year enabled false is respected",
			content:     minimalValid + "new_year:\n  enabled: false\n",
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

			if cfg.NewYear.Enabled == nil || *cfg.NewYear.Enabled != testCase.wantEnabled {
				t.Errorf("NewYear.Enabled = %v, want %v", cfg.NewYear.Enabled, testCase.wantEnabled)
			}
		})
	}
}

// TestLoadBirthdays covers birthday filtering (bad date skipped) and default application.
func TestLoadBirthdays(t *testing.T) {
	t.Parallel()

	t.Run("bad date_of_birth is skipped, valid entry is kept", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `birthdays:
  - date_of_birth: "not-a-date"
    name: "Bad"
  - date_of_birth: "1990-04-16"
    name: "Alice"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.Birthdays) != 1 {
			t.Fatalf("len(Birthdays) = %d, want 1", len(cfg.Birthdays))
		}

		if cfg.Birthdays[0].Name != "Alice" {
			t.Errorf("Birthdays[0].Name = %q, want \"Alice\"", cfg.Birthdays[0].Name)
		}
	})

	t.Run("defaults applied to birthday entry", func(t *testing.T) {
		t.Parallel()

		content := minimalValid + `birthdays:
  - date_of_birth: "1990-04-16"
    name: "Alice"
`

		cfg, err := config.Load(writeConfigFile(t, content))
		if err != nil {
			t.Fatalf("Load() unexpected error: %v", err)
		}

		if len(cfg.Birthdays) != 1 {
			t.Fatalf("len(Birthdays) = %d, want 1", len(cfg.Birthdays))
		}

		birthday := cfg.Birthdays[0]

		if birthday.Duration != config.DefaultBirthdayDuration {
			t.Errorf("Duration = %d, want %d", birthday.Duration, config.DefaultBirthdayDuration)
		}

		if birthday.Icon != config.DefaultBirthdayIcon {
			t.Errorf("Icon = %q, want %q", birthday.Icon, config.DefaultBirthdayIcon)
		}

		if birthday.Rainbow == nil || !*birthday.Rainbow {
			t.Error("Rainbow should default to true")
		}

		if birthday.Wakeup == nil || !*birthday.Wakeup {
			t.Error("Wakeup should default to true")
		}

		if birthday.ScrollSpeed != config.DefaultBirthdayScrollSpeed {
			t.Errorf(
				"ScrollSpeed = %d, want %d",
				birthday.ScrollSpeed,
				config.DefaultBirthdayScrollSpeed,
			)
		}
	})
}
