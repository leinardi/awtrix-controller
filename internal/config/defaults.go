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

	// DefaultScheduledNotificationDuration is the notification display time in seconds.
	DefaultScheduledNotificationDuration = 60

	// DefaultScheduledNotificationIcon is the LaMetric icon ID for scheduled notifications.
	DefaultScheduledNotificationIcon = "9597"

	// DefaultScheduledNotificationScrollSpeed is the scroll speed in px/frame for scheduled notifications.
	DefaultScheduledNotificationScrollSpeed = 50

	// DefaultScheduledNotificationTime is the default fire time (HH:MM).
	DefaultScheduledNotificationTime = "00:00"

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

	// DefaultWeatherPollIntervalMinutes is the polling interval for Open-Meteo forecasts.
	DefaultWeatherPollIntervalMinutes = 15

	// DefaultWeatherOverlayHorizonMinutes is the look-ahead window for overlay selection.
	DefaultWeatherOverlayHorizonMinutes = 60

	// DefaultWeatherNotificationHorizonHours is the look-ahead window for event detection.
	DefaultWeatherNotificationHorizonHours = 8

	// DefaultWeatherNotificationRepeatMinutes is the minimum interval between repeat notifications.
	DefaultWeatherNotificationRepeatMinutes = 60

	// DefaultWeatherInactiveAfterMissingPolls is the number of consecutive missing polls before
	// an event is considered inactive.
	DefaultWeatherInactiveAfterMissingPolls = 2

	// DefaultWeatherDataStaleTTLMinutes is the maximum age of fetched data before it is discarded.
	DefaultWeatherDataStaleTTLMinutes = 45

	// DefaultWeatherNotificationTextRepeat is the number of times notification text scrolls.
	DefaultWeatherNotificationTextRepeat = 3

	// DefaultWeatherGustWarnKmh is the wind gust threshold for a warning.
	DefaultWeatherGustWarnKmh = 45.0

	// DefaultWeatherGustSevereKmh is the wind gust threshold for a severe warning.
	DefaultWeatherGustSevereKmh = 60.0

	// DefaultWeatherHeavyRainMmPer15Min is the precipitation threshold for heavy rain.
	DefaultWeatherHeavyRainMmPer15Min = 5.0

	// DefaultWeatherFogVisibilityWarnM is the visibility threshold for a fog warning.
	DefaultWeatherFogVisibilityWarnM = 1000.0

	// DefaultWeatherFogVisibilitySevereM is the visibility threshold for a severe fog warning.
	DefaultWeatherFogVisibilitySevereM = 200.0

	// DefaultWeatherFrostTempC is the temperature threshold for frost risk.
	DefaultWeatherFrostTempC = 1.5

	// DefaultWeatherFrostDewPointDeltaC is the temperature-dewpoint delta for frost risk.
	DefaultWeatherFrostDewPointDeltaC = 1.0

	// DefaultWeatherFrostWarnPrecipWindowH is the look-forward window (hours) used to gate
	// frost-risk warnings: at least DefaultWeatherFrostWarnPrecipMm must fall within this window.
	DefaultWeatherFrostWarnPrecipWindowH = 2.0

	// DefaultWeatherFrostWarnPrecipMm is the minimum precipitation (mm) required within the
	// look-forward window to allow a frost-risk warning to fire.
	DefaultWeatherFrostWarnPrecipMm = 0.2
)
