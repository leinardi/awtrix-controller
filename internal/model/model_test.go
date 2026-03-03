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

package model_test

import (
	"bytes"
	"encoding/json"
	"reflect"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/model"
)

// ptr returns a pointer to the given int value.
func ptr(v int) *int { return &v }

// ptrBool returns a pointer to the given bool value.
func ptrBool(v bool) *bool { return &v }

// mustMarshal marshals v to JSON and fails the test on error.
func mustMarshal(t *testing.T, v any) []byte {
	t.Helper()

	data, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("json.Marshal: %v", err)
	}

	return data
}

// mustUnmarshal unmarshals data into dst and fails the test on error.
func mustUnmarshal(t *testing.T, data []byte, dst any) {
	t.Helper()

	err := json.Unmarshal(data, dst)
	if err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
}

// TestSettingsBriAbsentWhenNil verifies that BRI is absent from the JSON output
// when Bri is nil (the zero value for a pointer).
func TestSettingsBriAbsentWhenNil(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{})

	if strings.Contains(string(jsonData), `"BRI"`) {
		t.Errorf("expected BRI to be absent; got %s", jsonData)
	}
}

// TestSettingsBriPresentWhenSet verifies that BRI appears in the JSON output
// when Bri is set to a non-nil pointer.
func TestSettingsBriPresentWhenSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{Bri: ptr(1)})

	if !strings.Contains(string(jsonData), `"BRI":1`) {
		t.Errorf("expected BRI:1 in output; got %s", jsonData)
	}
}

// TestSettingsAbriAbsentWhenNil verifies that ABRI is absent when Abri is nil.
func TestSettingsAbriAbsentWhenNil(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{})

	if strings.Contains(string(jsonData), `"ABRI"`) {
		t.Errorf("expected ABRI to be absent; got %s", jsonData)
	}
}

// TestSettingsAbriFalsePresentWhenPointerSet verifies that ABRI:false is included
// in the JSON output when Abri is a pointer to false. Without the pointer type,
// omitempty would suppress a false bool value.
func TestSettingsAbriFalsePresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{Abri: ptrBool(false)})

	if !strings.Contains(string(jsonData), `"ABRI":false`) {
		t.Errorf("expected ABRI:false in output; got %s", jsonData)
	}
}

// TestSettingsColorFieldsOmittedWhenEmpty verifies that the six theme color fields
// managed by the application are absent from the JSON payload when not set.
func TestSettingsColorFieldsOmittedWhenEmpty(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{})

	for _, key := range []string{"CHCOL", "CBCOL", "WDCA", "WDCI", "TIME_COL", "DATE_COL"} {
		if strings.Contains(string(jsonData), `"`+key+`"`) {
			t.Errorf("expected %s to be absent; got %s", key, jsonData)
		}
	}
}

// TestStatsRoundTrip verifies that a fully-populated Stats struct survives a
// marshal→unmarshal cycle with all field values preserved.
func TestStatsRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.Stats{
		App:        "clock",
		Bat:        85,
		BatRaw:     712,
		Bri:        128,
		Hum:        55,
		Indicator1: true,
		Indicator2: false,
		Indicator3: true,
		IPAddress:  "192.168.1.42",
		LdrRaw:     400,
		Lux:        210,
		Matrix:     true,
		Messages:   3,
		Ram:        102400,
		Temp:       22,
		Type:       1,
		UID:        "AABBCCDD1122",
		Uptime:     3600,
		Version:    "0.96",
		WifiSignal: -65,
	}

	jsonData := mustMarshal(t, original)

	var got model.Stats
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestNotificationRoundTrip verifies that a Notification with several fields set
// survives a marshal→unmarshal cycle.
func TestNotificationRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.Notification{
		Text:        "Happy Birthday!",
		Icon:        "14004",
		Duration:    10,
		ScrollSpeed: 50,
		Draw:        []model.DrawCommand{{"dp": []any{3, 4, "#FF0000"}}},
	}

	jsonData := mustMarshal(t, original)

	var got model.Notification
	mustUnmarshal(t, jsonData, &got)

	// Re-marshal both and compare to handle map ordering inside DrawCommand.
	wantJSON := mustMarshal(t, original)
	gotJSON := mustMarshal(t, got)

	if !bytes.Equal(wantJSON, gotJSON) {
		t.Errorf("round-trip mismatch\nwant: %s\ngot:  %s", wantJSON, gotJSON)
	}
}

// TestCustomAppRoundTrip verifies that embedded Notification fields survive a
// marshal→unmarshal cycle alongside CustomApp-specific fields.
func TestCustomAppRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.CustomApp{
		Notification: model.Notification{
			Text: "My App",
			Icon: "1234",
		},
		Pos:      2,
		Lifetime: 30,
		Save:     true,
	}

	jsonData := mustMarshal(t, original)

	var got model.CustomApp
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawCommandMarshal verifies that a DrawCommand marshals to the expected
// JSON structure used by the Awtrix3 firmware.
func TestDrawCommandMarshal(t *testing.T) {
	t.Parallel()

	cmd := model.DrawCommand{"dp": []any{3, 4, "#FF0000"}}
	jsonData := mustMarshal(t, cmd)

	want := `{"dp":[3,4,"#FF0000"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestPowerFalseIsPresent verifies that Power{Power: false} marshals to
// {"power":false} and not to {}, confirming omitempty is intentionally absent.
func TestPowerFalseIsPresent(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Power{Power: false})

	want := `{"power":false}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestSleepZeroIsPresent verifies that Sleep{Sleep: 0} marshals to
// {"sleep":0} and not to {}, confirming that 0 (sleep indefinitely) is preserved.
func TestSleepZeroIsPresent(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Sleep{Sleep: 0})

	want := `{"sleep":0}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}
