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

package clientstate_test

import (
	"sync"
	"testing"

	"github.com/leinardi/awtrix-controller/internal/clientstate"
	"github.com/leinardi/awtrix-controller/internal/model"
)

func TestConnectedIDs_SortedAfterRegisterTwo(t *testing.T) {
	t.Parallel()

	r := clientstate.NewRegistry()
	r.Register("zebra")
	r.Register("alpha")

	ids := r.ConnectedIDs()

	if len(ids) != 2 {
		t.Fatalf("expected 2 IDs, got %d", len(ids))
	}

	if ids[0] != "alpha" || ids[1] != "zebra" {
		t.Errorf("expected [alpha zebra], got %v", ids)
	}
}

// TC-08: updating stats for one client must not affect the other.
func TestUpdateStats_IndependentClients(t *testing.T) {
	t.Parallel()

	reg := clientstate.NewRegistry()
	reg.Register("client-a")
	reg.Register("client-b")

	statsA := &model.Stats{App: "clockface", Bat: 90}
	reg.UpdateStats("client-a", statsA)

	snapA, okA := reg.Snapshot("client-a")
	snapB, okB := reg.Snapshot("client-b")

	if !okA {
		t.Fatal("client-a should be registered")
	}

	if !okB {
		t.Fatal("client-b should be registered")
	}

	if snapA.Stats == nil || snapA.Stats.App != "clockface" {
		t.Errorf("client-a stats mismatch: %+v", snapA.Stats)
	}

	if snapB.Stats != nil {
		t.Errorf("client-b stats should be nil, got %+v", snapB.Stats)
	}
}

// TC-16: unregistering a client must remove it from ConnectedIDs.
func TestUnregister_RemovesClient(t *testing.T) {
	t.Parallel()

	r := clientstate.NewRegistry()
	r.Register("dev-1")
	r.Register("dev-2")
	r.Unregister("dev-1")

	ids := r.ConnectedIDs()

	for _, id := range ids {
		if id == "dev-1" {
			t.Error("dev-1 should have been removed")
		}
	}

	if len(ids) != 1 || ids[0] != "dev-2" {
		t.Errorf("expected [dev-2], got %v", ids)
	}
}

func TestUpdateStats_UnknownClient_NoPanic(t *testing.T) {
	t.Parallel()

	r := clientstate.NewRegistry()
	// Must not panic; warning is logged but we cannot assert it without a test logger here.
	r.UpdateStats("ghost", &model.Stats{})
}

func TestSnapshot_UnknownClient_ReturnsFalse(t *testing.T) {
	t.Parallel()

	r := clientstate.NewRegistry()
	snap, ok := r.Snapshot("nonexistent")

	if ok || snap != nil {
		t.Errorf("expected (nil, false), got (%v, %v)", snap, ok)
	}
}

func TestUpdateCurrentApp(t *testing.T) {
	t.Parallel()

	r := clientstate.NewRegistry()
	r.Register("dev-1")
	r.UpdateCurrentApp("dev-1", "clock")

	snap, ok := r.Snapshot("dev-1")

	if !ok {
		t.Fatal("dev-1 should be registered")
	}

	if snap.CurrentApp != "clock" {
		t.Errorf("expected CurrentApp=clock, got %q", snap.CurrentApp)
	}
}

func TestSnapshot_ReturnsCopy(t *testing.T) {
	t.Parallel()

	reg := clientstate.NewRegistry()
	reg.Register("dev-1")
	reg.UpdateCurrentApp("dev-1", "original")

	snap, _ := reg.Snapshot("dev-1")
	snap.CurrentApp = "mutated"

	snap2, _ := reg.Snapshot("dev-1")

	if snap2.CurrentApp != "original" {
		t.Errorf("mutation of snapshot should not affect registry; got %q", snap2.CurrentApp)
	}
}

// Concurrent access — verified by the race detector (-race flag).
func TestConcurrentAccess(t *testing.T) {
	t.Parallel()

	reg := clientstate.NewRegistry()

	const concurrency = 50

	var waitGroup sync.WaitGroup

	waitGroup.Add(concurrency * 4)

	for idx := range concurrency {
		clientID := "client-" + string(rune('A'+idx%26))

		go func() {
			defer waitGroup.Done()

			reg.Register(clientID)
		}()

		go func() {
			defer waitGroup.Done()

			reg.UpdateStats(clientID, &model.Stats{Bat: idx})
		}()

		go func() {
			defer waitGroup.Done()

			reg.ConnectedIDs()
		}()

		go func() {
			defer waitGroup.Done()

			reg.Snapshot(clientID)
		}()
	}

	waitGroup.Wait()
}
