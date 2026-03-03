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

package model_test

import (
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
		AppContent: model.AppContent{
			Text:        model.NewPlainText("Happy Birthday!"),
			Icon:        "14004",
			Duration:    10,
			ScrollSpeed: 50,
			Draw:        model.DrawList{model.NewDrawPixel(3, 4, "#FF0000")},
		},
	}

	jsonData := mustMarshal(t, original)

	var got model.Notification
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestCustomAppRoundTrip verifies that embedded Notification fields survive a
// marshal→unmarshal cycle alongside CustomApp-specific fields.
func TestCustomAppRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.CustomApp{
		AppContent: model.AppContent{
			Text: model.NewPlainText("My App"),
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

// TestDrawPixelMarshal verifies that DrawPixel marshals to the expected JSON.
func TestDrawPixelMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawPixel(3, 4, "#FF0000"))

	want := `{"dp":[3,4,"#FF0000"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawLineMarshal verifies that DrawLine marshals to the expected JSON.
func TestDrawLineMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawLine(0, 0, 31, 7, "#00FF00"))

	want := `{"dl":[0,0,31,7,"#00FF00"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawRectMarshal verifies that DrawRect marshals to the expected JSON.
func TestDrawRectMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawRect(2, 2, 10, 5, "#FFFFFF"))

	want := `{"dr":[2,2,10,5,"#FFFFFF"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawFilledRectMarshal verifies that DrawFilledRect marshals to the expected JSON.
func TestDrawFilledRectMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawFilledRect(0, 0, 8, 8, "#0000FF"))

	want := `{"df":[0,0,8,8,"#0000FF"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawCircleMarshal verifies that DrawCircle marshals to the expected JSON.
func TestDrawCircleMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawCircle(5, 5, 3, "#FF00FF"))

	want := `{"dc":[5,5,3,"#FF00FF"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawFilledCircleMarshal verifies that DrawFilledCircle marshals to the expected JSON.
func TestDrawFilledCircleMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawFilledCircle(5, 5, 3, "#00FFFF"))

	want := `{"dfc":[5,5,3,"#00FFFF"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawTextMarshal verifies that DrawText marshals to the expected JSON.
func TestDrawTextMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawText(0, 0, "Hi", "#FFFFFF"))

	want := `{"dt":[0,0,"Hi","#FFFFFF"]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawBitmapMarshal verifies that DrawBitmap marshals to the expected JSON.
func TestDrawBitmapMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewDrawBitmap(0, 0, 2, 1, []int{16711680, 65280}))

	want := `{"db":[0,0,2,1,[16711680,65280]]}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawListMarshal verifies that a two-command DrawList marshals to the expected JSON array.
func TestDrawListMarshal(t *testing.T) {
	t.Parallel()

	drawList := model.DrawList{
		model.NewDrawPixel(1, 2, "#FF0000"),
		model.NewDrawLine(0, 0, 5, 5, "#00FF00"),
	}
	jsonData := mustMarshal(t, drawList)

	want := `[{"dp":[1,2,"#FF0000"]},{"dl":[0,0,5,5,"#00FF00"]}]`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestDrawPixelRoundTrip verifies DrawPixel survives a marshal→unmarshal cycle.
func TestDrawPixelRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawPixel(3, 4, "#FF0000")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawLineRoundTrip verifies DrawLine survives a marshal→unmarshal cycle.
func TestDrawLineRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawLine(0, 0, 31, 7, "#00FF00")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawRectRoundTrip verifies DrawRect survives a marshal→unmarshal cycle.
func TestDrawRectRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawRect(2, 2, 10, 5, "#FFFFFF")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawFilledRectRoundTrip verifies DrawFilledRect survives a marshal→unmarshal cycle.
func TestDrawFilledRectRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawFilledRect(0, 0, 8, 8, "#0000FF")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawCircleRoundTrip verifies DrawCircle survives a marshal→unmarshal cycle.
func TestDrawCircleRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawCircle(5, 5, 3, "#FF00FF")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawFilledCircleRoundTrip verifies DrawFilledCircle survives a marshal→unmarshal cycle.
func TestDrawFilledCircleRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawFilledCircle(5, 5, 3, "#00FFFF")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawTextRoundTrip verifies DrawText survives a marshal→unmarshal cycle.
func TestDrawTextRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawText(0, 0, "Hi", "#FFFFFF")}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawBitmapRoundTrip verifies DrawBitmap survives a marshal→unmarshal cycle.
func TestDrawBitmapRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{model.NewDrawBitmap(0, 0, 2, 1, []int{16711680, 65280})}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawListRoundTrip verifies a multi-command DrawList survives a marshal→unmarshal cycle.
func TestDrawListRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.DrawList{
		model.NewDrawPixel(1, 2, "#FF0000"),
		model.NewDrawLine(0, 0, 31, 7, "#00FF00"),
		model.NewDrawRect(2, 2, 10, 5, "#FFFFFF"),
		model.NewDrawFilledRect(0, 0, 8, 8, "#0000FF"),
		model.NewDrawCircle(5, 5, 3, "#FF00FF"),
		model.NewDrawFilledCircle(5, 5, 3, "#00FFFF"),
		model.NewDrawText(0, 0, "Hi", "#FFFFFF"),
		model.NewDrawBitmap(0, 0, 2, 1, []int{16711680, 65280}),
	}
	jsonData := mustMarshal(t, original)

	var got model.DrawList
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original, got) {
		t.Errorf("round-trip mismatch\nwant: %+v\ngot:  %+v", original, got)
	}
}

// TestDrawListUnmarshalUnknownKey verifies that an unknown draw command key returns an error.
func TestDrawListUnmarshalUnknownKey(t *testing.T) {
	t.Parallel()

	var drawList model.DrawList

	unmarshalErr := json.Unmarshal([]byte(`[{"zz":[1,2,"#FF"]}]`), &drawList)
	if unmarshalErr == nil {
		t.Error("expected error for unknown key, got nil")
	}
}

// TestDrawListUnmarshalWrongArgCount verifies that a draw command with too few args returns an error.
func TestDrawListUnmarshalWrongArgCount(t *testing.T) {
	t.Parallel()

	var drawList model.DrawList

	unmarshalErr := json.Unmarshal([]byte(`[{"dp":[1,2]}]`), &drawList)
	if unmarshalErr == nil {
		t.Error("expected error for wrong arg count, got nil")
	}
}

// TestDrawListUnmarshalBadArgType verifies that a draw command with wrong arg type returns an error.
func TestDrawListUnmarshalBadArgType(t *testing.T) {
	t.Parallel()

	var drawList model.DrawList

	unmarshalErr := json.Unmarshal([]byte(`[{"dp":[1,2,99]}]`), &drawList)
	if unmarshalErr == nil {
		t.Error("expected error for bad arg type, got nil")
	}
}

// TestDrawListUnmarshalNotArray verifies that a non-array input returns an error.
func TestDrawListUnmarshalNotArray(t *testing.T) {
	t.Parallel()

	var drawList model.DrawList

	unmarshalErr := json.Unmarshal([]byte(`"not-an-array"`), &drawList)
	if unmarshalErr == nil {
		t.Error("expected error for non-array input, got nil")
	}
}

// TestDrawListUnmarshalMultiKey verifies that a draw command object with multiple keys returns an error.
func TestDrawListUnmarshalMultiKey(t *testing.T) {
	t.Parallel()

	var drawList model.DrawList

	unmarshalErr := json.Unmarshal(
		[]byte(`[{"dp":[3,4,"#FF0000"],"dl":[0,0,1,1,"#FF0000"]}]`),
		&drawList,
	)
	if unmarshalErr == nil {
		t.Error("expected error for multi-key command object, got nil")
	}
}

// TestDrawInstructionInterfaceSatisfied verifies at compile-time that all concrete
// draw types implement the DrawInstruction interface.
func TestDrawInstructionInterfaceSatisfied(t *testing.T) {
	t.Parallel()

	var (
		_ model.DrawInstruction = (*model.DrawPixel)(nil)
		_ model.DrawInstruction = (*model.DrawLine)(nil)
		_ model.DrawInstruction = (*model.DrawRect)(nil)
		_ model.DrawInstruction = (*model.DrawFilledRect)(nil)
		_ model.DrawInstruction = (*model.DrawCircle)(nil)
		_ model.DrawInstruction = (*model.DrawFilledCircle)(nil)
		_ model.DrawInstruction = (*model.DrawText)(nil)
		_ model.DrawInstruction = (*model.DrawBitmap)(nil)
	)
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

// TestTransitionEffectRandomPresentWhenPointerSet verifies that TEFF:0 (Random) is included
// in the JSON output when Teff is a pointer to TransitionEffectRandom. Without the pointer
// type, omitempty would suppress the zero value.
func TestTransitionEffectRandomPresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	effect := model.TransitionEffectRandom
	jsonData := mustMarshal(t, model.Settings{Teff: &effect})

	if !strings.Contains(string(jsonData), `"TEFF":0`) {
		t.Errorf("expected TEFF:0 in output; got %s", jsonData)
	}
}

// TestTransitionEffectAbsentWhenNil verifies that TEFF is absent when Teff is nil.
func TestTransitionEffectAbsentWhenNil(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{})

	if strings.Contains(string(jsonData), `"TEFF"`) {
		t.Errorf("expected TEFF to be absent; got %s", jsonData)
	}
}

// TestTimeFormatMarshal verifies that a TimeFormat value marshals to its string literal.
func TestTimeFormatMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{Tformat: model.TimeFormat24h})

	if !strings.Contains(string(jsonData), `"TFORMAT":"%H:%M"`) {
		t.Errorf("expected TFORMAT:%%H:%%M in output; got %s", jsonData)
	}
}

// TestDateFormatMarshal verifies that a DateFormat value marshals to its string literal.
func TestDateFormatMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{Dformat: model.DateFormatDMY})

	if !strings.Contains(string(jsonData), `"DFORMAT":"%d.%m.%y"`) {
		t.Errorf("expected DFORMAT:%%d.%%m.%%y in output; got %s", jsonData)
	}
}

// TestOverlayEffectMarshalInSettings verifies that an OverlayEffect marshals correctly in Settings.
func TestOverlayEffectMarshalInSettings(t *testing.T) {
	t.Parallel()

	overlaySnow := model.OverlayEffectSnow
	jsonData := mustMarshal(t, model.Settings{Overlay: &overlaySnow})

	if !strings.Contains(string(jsonData), `"OVERLAY":"snow"`) {
		t.Errorf("expected OVERLAY:snow in output; got %s", jsonData)
	}
}

// TestOverlayEffectMarshalInNotification verifies that OverlayEffect is the shared type
// used by both Settings and Notification.
func TestOverlayEffectMarshalInNotification(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{Overlay: model.OverlayEffectRain}},
	)

	if !strings.Contains(string(jsonData), `"overlay":"rain"`) {
		t.Errorf("expected overlay:rain in output; got %s", jsonData)
	}
}

// TestPushIconBehaviorMarshal verifies that PushIconScroll marshals to pushIcon:1.
func TestPushIconBehaviorMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{PushIcon: model.PushIconScroll}},
	)

	if !strings.Contains(string(jsonData), `"pushIcon":1`) {
		t.Errorf("expected pushIcon:1 in output; got %s", jsonData)
	}
}

// TestTextCaseModeMarshal verifies that TextCaseAsIs marshals to textCase:2.
func TestTextCaseModeMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{TextCase: model.TextCaseAsIs}},
	)

	if !strings.Contains(string(jsonData), `"textCase":2`) {
		t.Errorf("expected textCase:2 in output; got %s", jsonData)
	}
}

// TestLifetimeResetModeMarshal verifies that LifetimeResetOnView marshals to lifetimeMode:1.
func TestLifetimeResetModeMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.CustomApp{LifetimeMode: model.LifetimeResetOnView})

	if !strings.Contains(string(jsonData), `"lifetimeMode":1`) {
		t.Errorf("expected lifetimeMode:1 in output; got %s", jsonData)
	}
}

// TestCenterFalsePresentWhenPointerSet verifies that center:false is included
// in the JSON output when Center is a pointer to false. Without the pointer type,
// omitempty would suppress a false bool value.
func TestCenterFalsePresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{Center: ptrBool(false)}},
	)

	if !strings.Contains(string(jsonData), `"center":false`) {
		t.Errorf("expected center:false in output; got %s", jsonData)
	}
}

// TestAutoscaleFalsePresentWhenPointerSet verifies that autoscale:false is included
// in the JSON output when Autoscale is a pointer to false. Without the pointer type,
// omitempty would suppress a false bool value.
func TestAutoscaleFalsePresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{Autoscale: ptrBool(false)}},
	)

	if !strings.Contains(string(jsonData), `"autoscale":false`) {
		t.Errorf("expected autoscale:false in output; got %s", jsonData)
	}
}

// TestStackFalsePresentWhenPointerSet verifies that stack:false is included
// in the JSON output when Stack is a pointer to false. Without the pointer type,
// omitempty would suppress a false bool value (which means "don't stack").
func TestStackFalsePresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Notification{Stack: ptrBool(false)})

	if !strings.Contains(string(jsonData), `"stack":false`) {
		t.Errorf("expected stack:false in output; got %s", jsonData)
	}
}

// TestProgressZeroPresentWhenPointerSet verifies that progress:0 is included
// in the JSON output when Progress is a pointer to 0. Without the pointer type,
// omitempty would suppress 0, hiding the "show 0% bar" intent.
func TestProgressZeroPresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Notification{AppContent: model.AppContent{Progress: ptr(0)}})

	if !strings.Contains(string(jsonData), `"progress":0`) {
		t.Errorf("expected progress:0 in output; got %s", jsonData)
	}
}

// TestRepeatZeroPresentWhenPointerSet verifies that repeat:0 is included
// in the JSON output when Repeat is a pointer to 0. Without the pointer type,
// omitempty would suppress 0, hiding the "scroll once" intent.
func TestRepeatZeroPresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Notification{AppContent: model.AppContent{Repeat: ptr(0)}})

	if !strings.Contains(string(jsonData), `"repeat":0`) {
		t.Errorf("expected repeat:0 in output; got %s", jsonData)
	}
}

// TestBarBCMarshal verifies that the barBC field serializes correctly.
func TestBarBCMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Notification{AppContent: model.AppContent{BarBC: "#00FF00"}})

	if !strings.Contains(string(jsonData), `"barBC":"#00FF00"`) {
		t.Errorf("expected barBC:#00FF00 in output; got %s", jsonData)
	}
}

// TestEffectMarshalInNotification verifies that an Effect constant marshals to its string value.
func TestEffectMarshalInNotification(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(
		t,
		model.Notification{AppContent: model.AppContent{Effect: model.EffectPlasma}},
	)

	if !strings.Contains(string(jsonData), `"effect":"Plasma"`) {
		t.Errorf("expected effect:Plasma in output; got %s", jsonData)
	}
}

// TestEffectPaletteMarshalInEffectSettings verifies that an EffectPalette constant marshals correctly.
func TestEffectPaletteMarshalInEffectSettings(t *testing.T) {
	t.Parallel()

	settings := model.EffectSettings{Palette: model.EffectPaletteRainbow}
	jsonData := mustMarshal(t, settings)

	if !strings.Contains(string(jsonData), `"palette":"Rainbow"`) {
		t.Errorf("expected palette:Rainbow in output; got %s", jsonData)
	}
}

// TestClockModeMarshalAbsentWhenNil verifies that TMODE is absent when Tmode is nil.
func TestClockModeMarshalAbsentWhenNil(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Settings{})

	if strings.Contains(string(jsonData), `"TMODE"`) {
		t.Errorf("expected TMODE to be absent; got %s", jsonData)
	}
}

// TestClockModeWeekdayBarPresentWhenPointerSet verifies that TMODE:0 (ClockModeWeekdayBar)
// is included when Tmode is a pointer to zero. Without the pointer type, omitempty would
// suppress the zero value.
func TestClockModeWeekdayBarPresentWhenPointerSet(t *testing.T) {
	t.Parallel()

	clockMode := model.ClockModeWeekdayBar
	jsonData := mustMarshal(t, model.Settings{Tmode: &clockMode})

	if !strings.Contains(string(jsonData), `"TMODE":0`) {
		t.Errorf("expected TMODE:0 in output; got %s", jsonData)
	}
}

// TestClockModeCalendarMarshal verifies that ClockModeCalendar(1) marshals to TMODE:1.
func TestClockModeCalendarMarshal(t *testing.T) {
	t.Parallel()

	clockMode := model.ClockModeCalendar
	jsonData := mustMarshal(t, model.Settings{Tmode: &clockMode})

	if !strings.Contains(string(jsonData), `"TMODE":1`) {
		t.Errorf("expected TMODE:1 in output; got %s", jsonData)
	}
}

// TestPlainTextMarshal verifies that NewPlainText marshals to a JSON string.
func TestPlainTextMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewPlainText("Hi"))

	want := `"Hi"`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestEmptyPlainTextMarshal verifies that NewPlainText("") marshals to an empty JSON string.
func TestEmptyPlainTextMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewPlainText(""))

	want := `""`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestFragmentedTextMarshal verifies that NewFragmentedText marshals to a JSON array.
func TestFragmentedTextMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.NewFragmentedText(
		model.TextFragment{Text: "Hello", Color: "FF0000"},
		model.TextFragment{Text: "!", Color: "00FF00"},
	))

	want := `[{"t":"Hello","c":"FF0000"},{"t":"!","c":"00FF00"}]`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestPlainTextRoundTrip verifies that a plain TextContent survives marshal→unmarshal.
func TestPlainTextRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.NewPlainText("Round trip!")

	jsonData := mustMarshal(t, original)

	var got model.TextContent
	mustUnmarshal(t, jsonData, &got)

	if got.Plain() != original.Plain() {
		t.Errorf("plain round-trip mismatch: want %q, got %q", original.Plain(), got.Plain())
	}
}

// TestFragmentedTextRoundTrip verifies that a fragmented TextContent survives marshal→unmarshal.
func TestFragmentedTextRoundTrip(t *testing.T) {
	t.Parallel()

	original := model.NewFragmentedText(
		model.TextFragment{Text: "Hello", Color: "FF0000"},
		model.TextFragment{Text: "World", Color: "0000FF"},
	)

	jsonData := mustMarshal(t, original)

	var got model.TextContent
	mustUnmarshal(t, jsonData, &got)

	if !reflect.DeepEqual(original.Fragments(), got.Fragments()) {
		t.Errorf(
			"fragment round-trip mismatch: want %v, got %v",
			original.Fragments(),
			got.Fragments(),
		)
	}
}

// TestTextContentUnmarshalInvalidJSON verifies that invalid JSON returns an error.
func TestTextContentUnmarshalInvalidJSON(t *testing.T) {
	t.Parallel()

	var textContent model.TextContent

	unmarshalErr := json.Unmarshal([]byte(`{invalid}`), &textContent)
	if unmarshalErr == nil {
		t.Error("expected error for invalid JSON, got nil")
	}
}

// TestNotificationWithFragmentedText verifies that a Notification with fragmented text
// marshals the "text" field as a JSON array.
func TestNotificationWithFragmentedText(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Notification{
		AppContent: model.AppContent{
			Text: model.NewFragmentedText(
				model.TextFragment{Text: "Hi", Color: "FF0000"},
			),
		},
	})

	if !strings.Contains(string(jsonData), `"text":[{"t":"Hi","c":"FF0000"}]`) {
		t.Errorf("expected fragmented text array; got %s", jsonData)
	}
}

// TestSoundMarshal verifies that Sound marshals to the expected JSON.
func TestSoundMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.Sound{Sound: "alarm"})

	want := `{"sound":"alarm"}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestAppSwitchMarshal verifies that AppSwitch marshals to the expected JSON.
func TestAppSwitchMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.AppSwitch{Name: "Time"})

	want := `{"name":"Time"}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestAppSwitchWithBuiltInApp verifies that BuiltInApp constants can be used as AppSwitch names.
func TestAppSwitchWithBuiltInApp(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.AppSwitch{Name: string(model.BuiltInAppTime)})

	want := `{"name":"Time"}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestBuiltInAppValues verifies the string values of all BuiltInApp constants.
func TestBuiltInAppValues(t *testing.T) {
	t.Parallel()

	tests := []struct {
		app  model.BuiltInApp
		want string
	}{
		{model.BuiltInAppTime, "Time"},
		{model.BuiltInAppDate, "Date"},
		{model.BuiltInAppTemperature, "Temperature"},
		{model.BuiltInAppHumidity, "Humidity"},
		{model.BuiltInAppBattery, "Battery"},
	}

	for _, tc := range tests {
		if string(tc.app) != tc.want {
			t.Errorf("BuiltInApp %q: want %q", tc.app, tc.want)
		}
	}
}

// TestMoodLightKelvinMarshal verifies that MoodLight with Kelvin marshals correctly.
func TestMoodLightKelvinMarshal(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.MoodLight{Brightness: 170, Kelvin: 2300})

	want := `{"brightness":170,"kelvin":2300}`
	if string(jsonData) != want {
		t.Errorf("want %s; got %s", want, jsonData)
	}
}

// TestMoodLightKelvinAbsentWhenZero verifies that the kelvin field is absent when zero.
func TestMoodLightKelvinAbsentWhenZero(t *testing.T) {
	t.Parallel()

	jsonData := mustMarshal(t, model.MoodLight{Brightness: 170, Color: "#FF0000"})

	if strings.Contains(string(jsonData), `"kelvin"`) {
		t.Errorf("expected kelvin to be absent; got %s", jsonData)
	}
}
