# Future Improvements

This document outlines planned enhancements for Blur AutoClicker macOS.

---

## High Priority

### 1. Human-like Smooth Mouse Movement
**Current state:** `smooth_move()` function exists in `mouse.rs` but is only activated when `smoothing == 1 && cps < 50.0`
**Problem:** Smoothing is hardcoded to be disabled at high CPS, limiting natural movement
**Improvement:** Add a dedicated "Human-like Mode" toggle that enables smooth cursor interpolation between clicks, even at higher CPS

### 2. Multi-Monitor Edge/Corner Detection
**Current state:** `current_screen_size()` in `failsafe.rs` only uses `CGDisplay::main()`, which returns the primary display
**Problem:** On multi-monitor setups, cursor position detection and edge/corner failsafe only work on the primary display
**Improvement:** Implement proper multi-monitor support using `CGGetDisplaysWithPoint()` to detect which display the cursor is on

### 3. Visual Overlay for Edge/Corner Zones
**Current state:** Edge/corner stops are invisible - users must guess the safe zones
**Improvement:** Add an optional visual overlay that shows colored zones at screen edges/corners based on configured threshold values

### 4. Profile System
**Current state:** All settings are saved to a single JSON file
**Improvement:** Allow users to save/load named profiles (e.g., "Gaming", "Work", "Afk Farm")

---

## Medium Priority

### 5. Light/Dark Mode Toggle
**Current state:** App only has dark mode (matches Windows original)
**Reference:** Windows v3.4.0 added light/dark mode toggle
**Improvement:** Add theme switching in settings

### 6. Keyboard Key Pressing (Not Just Mouse)
**Reference:** othyn/macos-auto-clicker supports keyboard pressing
**Improvement:** Add ability to press keyboard keys as part of click sequences

### 7. CPU Usage Measurement
**Current state:** `avg_cpu: -1.0` (hardcoded, not implemented on macOS)
**Reference:** Windows uses `QueryThreadCycleTime` for accurate CPU measurement
**Improvement:** Implement using `mach_thread_time` on macOS to measure actual clicker thread CPU usage

### 8. Macro/Sequence Clicking
**Current state:** Macro panel exists as UI stub ("coming eventually")
**Improvement:** Implement recording and playback of click/key sequences with configurable delays

### 9. Strict Hotkey Modifier Mode
**Reference:** Windows PR #96 added "Strict Hotkey Modifiers" toggle
**Current state:** Hotkey matching is one-way (extra modifiers ignored)
**Improvement:** Add a strict mode toggle in Settings for users who need exact modifier matching (e.g., `Ctrl+Y` vs `Ctrl+Shift+Y`)

---

## Low Priority

### 10. Multiple Language Support (i18n)
**Reference:** othyn/macos-auto-clicker has i18n
**Improvement:** Add translations for non-English users

### 11. Color Themes
**Improvement:** Allow custom color schemes beyond light/dark

### 12. Click Randomization Enhancement
**Reference:** Issue #95 on Windows - "Click Randomisation not random enough"
**Current state:** Uses `next_gaussian()` for Gaussian distribution
**Improvement:** Review RNG seeding and consider alternative randomization strategies

---

## Code Quality Improvements

### 13. React: Add Error Boundaries
**Current state:** No error boundaries - any component crash unmounts entire React tree
**Improvement:** Add `<ErrorBoundary>` around panel components

### 14. React: Split AdvancedPanelLayout.tsx (1007 lines)
**Current state:** One massive file with complex conditional rendering
**Improvement:** Extract sub-components: `ToggleBtn`, `Disableable`, `NumInput` should be in separate files

### 15. React: Memoize Panel Components
**Current state:** Panels re-render when parent re-renders even if props unchanged
**Improvement:** Wrap panel exports in `React.memo()`

### 16. Rust: Use SeqCst Ordering for Atomics
**Current state:** `CLICK_COUNT.load(Ordering::Relaxed)` used in multiple places
**Improvement:** Change to `SeqCst` for correctness

### 17. Rust: Add Input Validation
**Current state:** No validation on Rust side for settings fields
**Improvement:** Add range checks in `update_settings` command (e.g., click_speed > 0, duty_cycle <= 100)

### 18. Rust: Remove Dead Code
- `stats.rs`: `sent` field in `RunRecord` is set but never used for telemetry tracking
- Commented-out code in `SettingsPanel.tsx` and `store.ts`
- Duplicate `current_cursor_position()` in `mouse.rs` and `failsafe.rs`

### 19. Rust: Graceful Shutdown
**Current state:** `std::process::exit(0)` terminates abruptly
**Improvement:** Set `running.store(false)` and give thread time to finish before exit

### 20. Extract Magic Numbers
- `App.tsx:228` - `width = 800, height = 600`
- `App.tsx:237` - `wait(30)` - why 30ms?
- `worker.rs:312` - `* 1000.0` for hold_ms calculation

### 21. CSS Cleanup
- Inline styles in `TitleBar.tsx`, `MacroPanel.tsx`, `AdvancedPanelLayout.tsx`
- Consider using CSS modules or a styled-component approach

---

## Technical Debt

### 22. ESLint Configuration
**Current state:** ESLint v9 migration not complete - no `eslint.config.js`
**Improvement:** Migrate from `.eslintrc.*` to flat config format

### 23. Testing Infrastructure
**Current state:** No tests
**Improvement:** Add:
- Unit tests for settings validation
- Integration tests for mouse event generation
- Test edge/corner detection with multiple monitors

### 24. Telemetry Data Not Sent
**Current state:** `telemetry.rs` is implemented but disabled on macOS
**Improvement:** Either implement properly or remove dead telemetry code

### 25. MacroPanel Stub
**Current state:** Tab exists with "coming eventually" placeholder
**Improvement:** Either implement basic macro support or hide tab until ready

---

## Performance Optimization

### 26. React State Management
**Current state:** `useState` + `useRef` for complex settings state
**Improvement:** Consider `zustand` or `jotai` for simpler state management

### 27. Settings Debounce Refinement
**Current state:** 100ms debounce for saves, but `syncSettingsToBackend` could race
**Improvement:** Use a write queue or make `syncSettingsToBackend` also debounced

### 28. Monitor Work Area Clamping
**Reference:** Windows PR #96 fixes window cropping on smaller displays
**Current state:** App uses fixed 800x600 window
**Improvement:** Clamp window to active monitor work area on startup

---

## Security

### 29. Hotkey Normalization
**Current state:** `normalize_hotkey` could accept invalid key sequences
**Improvement:** Add validation for allowed key names

### 30. Settings Sanitization
**Improvement:** Ensure all user inputs are sanitized on the Rust side before use

---

*Last updated: 2026-04-18*