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

package config

// Default values applied by Validate when optional fields are absent.
const (
	// DefaultMQTTPort is the MQTT TCP listen port when mqtt.port is not specified.
	DefaultMQTTPort = 1883

	// DefaultConfigPath is the path to the YAML config file used when no
	// --config / -c flag or AWTRIX_CONFIG env var is provided.
	DefaultConfigPath = "/etc/awtrix-controller/config.yaml"

	// DefaultEnergySavingStart is the energy-saving window start time (HH:MM).
	DefaultEnergySavingStart = "00:30"

	// DefaultEnergySavingEnd is the energy-saving window end time (HH:MM).
	DefaultEnergySavingEnd = "06:00"

	// DefaultBirthdayDuration is the notification display time in seconds.
	DefaultBirthdayDuration = 600

	// DefaultBirthdayIcon is the LaMetric icon ID for birthday notifications.
	DefaultBirthdayIcon = "14004"

	// DefaultBirthdayScrollSpeed is the scroll speed in px/frame for birthday notifications.
	DefaultBirthdayScrollSpeed = 50

	// DefaultNewYearIcon is the LaMetric icon ID for New Year notifications.
	DefaultNewYearIcon = "5855"

	// DefaultNewYearDuration is the notification display time in seconds.
	DefaultNewYearDuration = 600

	// DefaultNewYearScrollSpeed is the scroll speed in px/frame for New Year notifications.
	DefaultNewYearScrollSpeed = 50

	// DefaultThemeDayAccent is the calendar accent color for the day theme.
	DefaultThemeDayAccent = "#FF0000"

	// DefaultThemeDayContent is the content (text/date) color for the day theme.
	DefaultThemeDayContent = "#FFFFFF"

	// DefaultThemeNightAccent is the calendar accent color for the night theme.
	DefaultThemeNightAccent = "#FF0000"

	// DefaultThemeNightContent is the content (text/date) color for the night theme.
	DefaultThemeNightContent = "#474747"

	// hexColorLen is the expected length of a #RRGGBB color string (1 hash + 6 hex digits).
	hexColorLen = 7
)
