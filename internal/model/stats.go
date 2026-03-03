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

// Package model defines JSON-serializable types representing MQTT payloads
// exchanged with Awtrix3 LED matrix display firmware. The package contains
// only type definitions — no behavior.
package model

// Stats represents the status payload published by the device on {clientId}/stats.
// JSON field names match the firmware's exact snake_case keys.
//
//nolint:tagliatelle // firmware publishes snake_case JSON keys; must match exactly
type Stats struct {
	App        string `json:"app"`
	Bat        int    `json:"bat"`
	BatRaw     int    `json:"bat_raw"`
	Bri        int    `json:"bri"`
	Hum        int    `json:"hum"`
	Indicator1 bool   `json:"indicator1"`
	Indicator2 bool   `json:"indicator2"`
	Indicator3 bool   `json:"indicator3"`
	IPAddress  string `json:"ip_address"`
	LdrRaw     int    `json:"ldr_raw"`
	Lux        int    `json:"lux"`
	Matrix     bool   `json:"matrix"`
	Messages   int    `json:"messages"`
	Ram        int    `json:"ram"`
	Temp       int    `json:"temp"`
	Type       int    `json:"type"`
	UID        string `json:"uid"`
	Uptime     int    `json:"uptime"`
	Version    string `json:"version"`
	WifiSignal int    `json:"wifi_signal"`
}
