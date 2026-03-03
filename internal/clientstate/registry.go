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

// Package clientstate provides a thread-safe registry of connected MQTT clients,
// tracking per-client stats and current app name.
package clientstate

import (
	"sort"
	"sync"

	"github.com/leinardi/awtrix-controller/internal/logger"
	"github.com/leinardi/awtrix-controller/internal/model"
)

// ClientState holds the last-known state for a single connected client.
type ClientState struct {
	Stats      *model.Stats
	CurrentApp string
}

// Registry is a thread-safe in-memory store of connected client states.
type Registry struct {
	mu      sync.RWMutex
	clients map[string]*ClientState
}

// NewRegistry returns an empty Registry ready for use.
func NewRegistry() *Registry {
	return &Registry{
		clients: make(map[string]*ClientState),
	}
}

// Register adds clientID with an empty ClientState. Calling Register for an
// already-registered client replaces its state.
func (r *Registry) Register(clientID string) {
	r.mu.Lock()
	defer r.mu.Unlock()

	r.clients[clientID] = &ClientState{}
}

// Unregister removes all state for clientID. It is a no-op if the client is
// not registered.
func (r *Registry) Unregister(clientID string) {
	r.mu.Lock()
	defer r.mu.Unlock()

	delete(r.clients, clientID)
}

// UpdateStats stores stats for clientID. If the client is not registered, the
// call is a no-op and a warning is logged.
func (r *Registry) UpdateStats(clientID string, stats *model.Stats) {
	r.mu.Lock()
	defer r.mu.Unlock()

	state, ok := r.clients[clientID]
	if !ok {
		logger.L().Warn("UpdateStats: client not registered", "clientID", clientID)

		return
	}

	state.Stats = stats
}

// UpdateCurrentApp stores the current app name for clientID. If the client is
// not registered, the call is a no-op.
func (r *Registry) UpdateCurrentApp(clientID, app string) {
	r.mu.Lock()
	defer r.mu.Unlock()

	state, ok := r.clients[clientID]
	if !ok {
		return
	}

	state.CurrentApp = app
}

// ConnectedIDs returns a sorted copy of all currently registered client IDs.
func (r *Registry) ConnectedIDs() []string {
	r.mu.RLock()
	defer r.mu.RUnlock()

	ids := make([]string, 0, len(r.clients))
	for id := range r.clients {
		ids = append(ids, id)
	}

	sort.Strings(ids)

	return ids
}

// Snapshot returns a shallow copy of the ClientState for clientID and true, or
// nil and false if the client is not registered.
func (r *Registry) Snapshot(clientID string) (*ClientState, bool) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	state, ok := r.clients[clientID]
	if !ok {
		return nil, false
	}

	snap := *state

	return &snap, true
}
