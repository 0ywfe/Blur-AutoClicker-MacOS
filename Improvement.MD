# Blur AutoClicker - Code Analysis & Improvement Report

## Architecture Overview

- **Frontend**: React 19 + TypeScript + Vite (Tauri v2)
- **Backend**: Rust with Core Graphics (CGEvent HID) for mouse events
- **State**: Frontend settings with tauri-plugin-store persistence, backend ClickerState managed via Arc<AtomicBool>/Mutex
- **Hotkeys**: tauri-plugin-global-shortcut

---

## Critical Issues

### 1. Tab Change Logic Bug (App.tsx:275-285)

```typescript
const handleTabChange = (nextTab: Tab) => {
  setTab(nextTab);

  if (nextTab === "settings") return;
  if (settingsRef.current.lastPanel === nextTab) return;

  persistSettings({
    ...settingsRef.current,
    lastPanel: nextTab,
  });
};
```

**Problem**: `persistSettings` updates `settingsRef` at the end, but the check `settingsRef.current.lastPanel === nextTab` runs immediately after `setTab(nextTab)` and before `persistSettings` completes. So `lastPanel` still holds the **old** value, making the condition always `false` on first tab change to a new panel.

**Fix**: Use `updateSettings` instead of `persistSettings` directly, or check against `settings.lastPanel` (the React state).

---

### 2. Top-Level Await in Store Module (store.ts:5)

```typescript
const store = new LazyStore("settings.json");
import { getVersion } from "@tauri-apps/api/app";
const version = await getVersion();
```

**Problem**: If `getVersion()` fails, the entire module fails to load, breaking all settings operations. There's no error handling.

**Fix**: Wrap in try/catch or initialize lazily.

---

### 3. Status Polling While Running (App.tsx:207-218)

```typescript
useEffect(() => {
  if (!status.running) return;

  const interval = window.setInterval(() => {
    invoke<ClickerStatus>("get_status")
      .then(setStatus)
      .catch(...);
  }, 200);

  return () => window.clearInterval(interval);
}, [status.running]);
```

**Problem**: Polls every 200ms when running. The backend already emits `clicker-status` events on state change (worker.rs:91 `emit_status`). Polling is redundant and wasteful.

**Fix**: Remove polling entirely and rely on backend events. If events are missed, that's a backend bug to fix, not a reason to poll.

---

### 4. Race Condition in Window Placement (App.tsx:230-250)

```typescript
useEffect(() => {
  const appWindow = getCurrentWindow();
  const width = 800;
  const height = 600;

  void (async () => {
    try {
      if (consentGiven === false) return;

      await appWindow.setSize(new LogicalSize(width, height));

      if (!settingsLoaded || launchWindowPlacementDone.current) return;
      await wait(30);
      await applyStartupWindowPlacement();

      launchWindowPlacementDone.current = true;
    } catch (err) {
      console.error("Failed to size or place window:", err);
    }
  })();
}, [settings, settingsLoaded, tab, consentGiven, updateInfo]);
```

**Problem**: This effect runs on multiple dependencies. If any changes, it re-runs and potentially overwrites previous placement. `launchWindowPlacementDone` prevents re-running but doesn't prevent concurrent executions.

**Fix**: Use a proper cleanup with a mutex-style flag or simplify to run only once on mount.

---

### 5. Hotkey Effect Has Stale Closure (App.tsx:151-187)

**Problem**: 250ms debounce uses `settings.hotkey` at the time the timeout fires, but `persistSettings` may have already updated `settingsRef.current` with a newer hotkey. The canonicalization result is then compared against stale data.

**Fix**: Track the hotkey value at the time of timeout creation and compare only against that stable reference.

---

### 6. No Error Boundaries in React

**Problem**: If any component throws, the entire React tree unmounts. No graceful degradation.

**Fix**: Add error boundaries around panel components.

---

## Moderate Issues

### 7. Multiple Parallel Async Ops Chain Dependencies Unnecessarily (App.tsx:107-144)

```typescript
Promise.all([loadSettings(), invoke<AppInfo>("get_app_info"), invoke<ClickerStatus>("get_status")])
  .then(async ([loadedSettings, loadedAppInfo, loadedStatus]) => {
    // ...
    await syncSettingsToBackend(hydratedSettings);  // sequential after Promise.all
    await invoke("register_hotkey", { hotkey: hydratedSettings.hotkey }); // sequential
    // ...
  })
```

**Problem**: `syncSettingsToBackend` and `register_hotkey` run sequentially inside `.then()`, but they could be parallelized since they don't depend on each other's results.

**Fix**: Use `Promise.all([syncSettingsToBackend(...), invoke("register_hotkey", ...)])`.

---

### 8. ClickCount Uses Relaxed Ordering (engine/mod.rs:63)

```typescript
pub static CLICK_COUNT: AtomicI64 = AtomicI64::new(0);
// ...
CLICK_COUNT.store(click_count, Ordering::Relaxed);
// ...
pub fn get_click_count() -> i64 {
    CLICK_COUNT.load(Ordering::Relaxed)
}
```

**Problem**: `Relaxed` ordering means no happens-before guarantee. Updates may not be visible to readers immediately.

**Fix**: Use `SeqCst` (sequentially consistent) ordering for correctness.

---

### 9. Settings Version Type Mismatch

**Problem**:
- Frontend `store.ts`: `version: string` (line 11)
- Backend `settings/mod.rs`: `version: u32` (line 4)
- App.tsx sends `version: 5` (hardcoded number, not string)

**Fix**: Align types. If version is numeric on backend, send numeric from frontend. If string, fix backend.

---

### 10. Hotkey Timer Not Cleaned Up on Unmount (App.tsx:63, 151-186)

```typescript
const hotkeyTimer = useRef<number | null>(null);
// ...
useEffect(() => {
  // Sets hotkeyTimer but no cleanup on component unmount
}, [settings.hotkey, settingsLoaded]);
```

**Problem**: If component unmounts while timer is pending, it fires with a potentially stale app handle.

**Fix**: Return cleanup function in the useEffect to clear the timer.

---

### 11. updateSettings Always Creates New Object (App.tsx:92-94)

```typescript
const updateSettings = (patch: Partial<Settings>) => {
  persistSettings({ ...settingsRef.current, ...patch });
};
```

**Problem**: Creates a new object even if `patch` has no changes. Causes downstream components to re-render even when nothing changed.

**Fix**: Add shallow comparison or only update if there are actual changes.

---

### 12. Duplicate Cursor Position Implementation

- `engine/mouse.rs:75-81` - `current_cursor_position()`
- `engine/failsafe.rs:9-15` - `current_cursor_position()`

**Problem**: Duplicated code. If one is fixed, the other may become outdated.

**Fix**: Create a shared utility or move to a common module.

---

### 13. MacroPanel is Stub Only (MacroPanel.tsx)

```typescript
export default function MacroPanel() {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%" }}>
      <span style={{ color: "var(--text-dim)", fontFamily: "'DMSans', sans-serif", fontSize: "16px" }}>
        Macro mode — coming eventually
      </span>
    </div>
  );
}
```

**Problem**: Tab exists but does nothing. User expectation mismatch.

**Fix**: Either implement basic macro support or hide the tab until ready.

---

### 14. Commented Code Never Cleaned (SettingsPanel.tsx:53, 172-182, 283)

```typescript
// const [resettingStats, setResettingStats] = useState(false);
// <button ...>...</button> {/* TODO: BUTTON DISABLED FOR NOW UNTIL I MAKE A CONFIRMATION PROMPT */}
// {/* TODO: BUTTON DISABLED FOR NOW UNTIL I MAKE A CONFIRMATION PROMPT */}
// {/* TODO: But since we're not accepting new telemetry connections... */} (telemetry.rs)
```

**Problem**: Dead code. Makes codebase harder to navigate.

**Fix**: Either implement or remove commented code.

---

### 15. Store Debounce Shares Timer But Not Ideal (App.tsx:84-89)

```typescript
saveTimerRef.current = setTimeout(() => {
  saveSettings(nextSettings).catch(...);
}, 100);
```

**Problem**: 100ms debounce is good, but multiple rapid settings changes could still cause race conditions with `syncSettingsToBackend`.

**Fix**: Consider making `syncSettingsToBackend` also debounced or use a write queue.

---

### 16. No Validation on Numeric Inputs (AdvancedPanelLayout.tsx:92-111)

The `NumInput` component does onChange and onBlur validation, but:
- Values can be set programmatically via `update({ clickSpeed: v })` without validation
- `maxDoubleClickDelayMs` logic is complex and could produce unexpected max values

**Fix**: Add centralized validation in `persistSettings` or `sanitizeSettings`.

---

### 17. ToggleBtn Internal useEffect (AdvancedPanelLayout.tsx:32-36)

```typescript
React.useEffect(() => {
  if (disabled && value) {
    onChange(false);
  }
}, [disabled, value, onChange]);
```

**Problem**: If `disabled` becomes true while `value` is true, this immediately calls `onChange(false)`. This is a side effect inside a pure UI component. The dependency array includes `onChange` which could cause infinite loops if the parent doesn't useCallback.

**Fix**: Move this logic to parent or use `useLayoutEffect` with proper memoization.

---

### 18. Update Check Interval Never Cleared (App.tsx:271)

```typescript
const interval = setInterval(checkForUpdates, 60 * 60 * 1000);
return () => clearInterval(interval);
```

**Problem**: The interval is cleared on unmount, but `checkForUpdates` captures `setUpdateInfo` in a closure. If component unmounts during `checkForUpdates`, there could be issues.

**Fix**: Use useCallback properly or add cleanup flag.

---

### 19. handleResetSettings Has No User Error Feedback (App.tsx:287-298)

```typescript
const handleResetSettings = async () => {
  try {
    const resetSettings = await invoke<Settings>("reset_settings");
    // ...
  } catch (err) {
    console.error("Failed to reset settings:", err);
  }
};
```

**Problem**: Errors are logged but not shown to user. User has no indication reset failed.

**Fix**: Add toast/alert UI for errors.

---

### 20. handlePickPosition Swallows Errors (App.tsx:300-311)

**Problem**: `invoke("pick_position")` could fail for various reasons (permissions, display issues), but user is never informed.

**Fix**: Show error to user via toast/alert.

---

### 21. No Loading States for Async Operations

Multiple operations lack loading indicators:
- `handleResetSettings`
- `handlePickPosition`
- Settings loading on boot

**Fix**: Add loading spinners or disable buttons during async operations.

---

### 22. Consent Dialog Doesn't Update Window Size Synchronously (App.tsx:222-228)

```typescript
useEffect(() => {
  if (consentGiven !== false || !settingsLoaded) return;
  getCurrentWindow()
    .setSize(new LogicalSize(420, 520))
    .then(() => getCurrentWindow().center())
    .catch((err) => console.error("Failed to size window for consent:", err));
}, [consentGiven, settingsLoaded]);
```

**Problem**: The consent dialog renders at full size before the resize completes. User sees a flash of the wrong size.

**Fix**: Show a loading state until resize completes, or render consent at correct size from the start.

---

### 22b. CumulativeStats Interface Mismatch (store.ts:7-13 vs stats.rs:13-18)

**Problem**: TypeScript interface expects `sessionsSent` and `avgCpu` (camelCase) but Rust returns `total_sessions` and `avg_cpu` (snake_case). The interface declares these fields but Rust doesn't populate `sessionsSent` at all. When the UI tries to display `stats.sessionsSent`, it's always `undefined` or `0`.

Frontend (store.ts):
```typescript
interface CumulativeStats {
  totalClicks: number;
  totalTimeSecs: number;
  totalSessions: number;
  sessionsSent: number;  // <-- never populated by backend
  avgCpu: number;
}
```

Backend (stats.rs):
```rust
pub struct CumulativeStats {
    pub total_clicks: i64,
    pub total_time_secs: f64,
    pub total_sessions: i64,
    pub avg_cpu: f64,
    // missing: sessions_sent
}
```

**Fix**: Add `sessions_sent` field to Rust `CumulativeStats` or remove from TypeScript interface. Also ensure serde aliasing is properly configured for camelCase conversion.

---

### 22c. Corner Failsafe Detection Logic Bug (failsafe.rs:27-41)

**Problem**: The corner detection uses wrong logic. It checks `cursor.0 <= config.corner_stop_tl && cursor.1 <= config.corner_stop_tl` for top-left, which means it uses `corner_stop_tl` value as both X (left) and Y (top) threshold. This is semantically incorrect — TL should check against left AND top thresholds separately.

```rust
if cursor.0 <= config.corner_stop_tl && cursor.1 <= config.corner_stop_tl {
    return Some(String::from("Top-left corner failsafe"));
}
```

This should be checking `cursor.x <= corner_stop_tl` (left threshold) AND `cursor.y <= corner_stop_tl` (top threshold). But the code uses the same `corner_stop_tl` value for both axes, which may be intentional if the config represents a single "radius" value, but then the naming is misleading.

For bottom-left:
```rust
if cursor.0 <= config.corner_stop_bl && cursor.1 >= screen.1 - config.corner_stop_bl {
```
This checks if cursor.x <= BL's left threshold AND cursor.y is within BL's bottom threshold. But if `corner_stop_bl` is meant to be a "distance from corner" value, this logic is backwards — you'd want to check if Y >= screen_height - bl_threshold, which is correct.

**Fix**: Clarify naming and document intent. Consider renaming to `corner_stop_left`, `corner_stop_right`, `corner_stop_top`, `corner_stop_bottom` for clarity, or document that each corner uses a single "radius" value for both axes.

---

### 22d. Stats `sent` Field is Never Used (stats.rs)

**Problem**: The `sent` field in `RunRecord` is set to `false` on creation (line 197) and read (line 123) but never actually used to track whether telemetry was sent. The telemetry is disabled on macOS anyway (telemetry.rs:48), so this is dead code.

**Fix**: Either implement telemetry send tracking or remove the `sent` field entirely.

---

### 22e. Reset Stats Button Has Misleading Comment (SettingsPanel.tsx:172-182)

```typescript
{/* TODO: BUTTON DISABLED FOR NOW UNTIL I MAKE A CONFIRMATION PROMPT */}
```

**Problem**: The comment says the button is disabled until a confirmation prompt is made, but:
1. The button IS actually commented out (not just disabled)
2. There's a `resettingStats` state commented out too
3. There's a `reset_stats` backend command that has no UI button to trigger

**Fix**: Either implement the reset stats feature with confirmation, or remove the commented button and backend command if it's not planned.

---

## Minor Issues

### 23. Comment Typos

- `TelemetryConsent.tsx:49` - "No thank's" should be "No thanks"
- `MacroPanel.tsx:6` - "coming eventually" should be "coming soon"

---

### 24. Magic Numbers Not Extracted

- `App.tsx:48` - `version: 5` hardcoded
- `App.tsx:250` - `wait(30)` - why 30ms?
- `App.tsx:233-234` - `width = 800, height = 600`
- `worker.rs:312` - `* 1000.0` for hold_ms calculation
- `mouse.rs:402` - `eprintln!` for debug (should use DEV_LOGGER)

---

### 25. AdvancedPanelLayout.tsx Very Long (1007 lines)

**Problem**: Single file with complex conditional rendering for compact/non-compact modes.

**Fix**: Consider splitting into smaller components or using a render prop pattern.

---

### 26. Panel Components Not Memoized (App.tsx:6-14)

```typescript
const SimplePanel = lazy(() => import("./components/panels/SimplePanel"));
// ...etc
```

**Problem**: Panels re-render when parent re-renders even if their props haven't changed.

**Fix**: Wrap exported components in `React.memo`.

---

### 27. Settings Fields Not Validated on Backend (settings/mod.rs)

Backend accepts any values without range checking. For example:
- `click_speed: f64` - could be 0 or negative
- `duty_cycle: f64` - could be > 100

**Fix**: Add validation in `update_settings` command.

---

### 28. Mouse Event Posting Errors Silently Caught (mouse.rs:68-69)

```typescript
for event in events {
  let _ = event.post(CGEventTapLocation::HID);
}
```

**Problem**: Posting errors are ignored. If events fail to post, user gets no feedback.

**Fix**: Log errors via DEV_LOGGER or surface to user.

---

### 29. Backend Exit Handler Uses Default Settings (lib.rs:99-110)

```rust
if settings.telemetry_enabled {
  let data = TelemetryData::from_settings(&settings, ...);
  // ...
}
std::process::exit(0);
```

**Problem**: On close, it uses `ClickerSettings::default()` instead of the actual settings from state.

**Fix**: Get settings from `app.state::<ClickerState>()`.

---

### 30. CSS inline styles scattered (TitleBar.tsx, MacroPanel.tsx, AdvancedPanelLayout.tsx)

**Problem**: Inline styles make maintenance harder. Should use CSS classes.

**Fix**: Use CSS classes or CSS-in-JS solution.

---

## Performance Suggestions

### 31. React.memo on Panel Components

Panel components receive `settings` and `update` props. Even with React.lazy, they re-render when App re-renders. Memoize them.

---

### 32. Avoid Object Spread in updateSettings

`{ ...settingsRef.current, ...patch }` always creates a new object, triggering re-renders even when values haven't changed.

---

### 33. Consider Using zustand or Jotai Instead of useState+useRef

For complex state like settings with multiple derived values, a state management library could reduce boilerplate and improve performance.

---

## Security Suggestions

### 34. No Input Sanitization on Rust Side

Frontend sends settings to backend, which deserializes them. No validation is performed on the Rust side before using values (e.g., array indices, coordinates).

---

### 35. Hotkey Normalization Could Allow Invalid Keys

`normalize_hotkey` and `parse_hotkey_binding` accept strings that might bypass some validations, leading to unexpected behavior.

---

### 36. No Graceful Shutdown for Clicker Thread (lib.rs, worker.rs)

**Problem**: When the app closes (lib.rs:91-117), if the clicker is running, the `running` AtomicBool is not set to `false` before the thread potentially continues. The `std::process::exit(0)` call at line 116 terminates abruptly without signaling the worker thread to stop. While the worker's while-loop checks `running.load()`, on macOS the thread might not get signaled before process termination.

**Fix**: On close, explicitly set `state.running.store(false, Ordering::SeqCst)` and give the worker thread a brief moment to finish before `process::exit`.

---

### 37. Hotkey Suppression Timing Could Miss Edge Cases (hotkeys.rs, ui_commands.rs)

**Problem**: `suppress_hotkey_until_ms` and `suppress_hotkey_until_release` are two separate mechanisms:
- `suppress_hotkey_until_ms`: suppress for 250ms after capture ends
- `suppress_hotkey_until_release`: suppress until key release

The interaction between these two is complex. If the user presses a key, captures it, then releases - the 250ms timer might expire while the key is still conceptually "held" from the system's perspective, potentially causing unwanted double-triggers.

**Fix**: Simplify the suppression logic or add integration tests for edge cases.

---

### 38. Stats CSV Stores Hash but Never Validates Integrity Before Use (stats.rs:139-144)

**Problem**: `verify_hash` is called during read (line 139) and logs a warning if verification fails, but the records are still used. There's no mechanism to reject or recover corrupted records — they accumulate in the CSV forever.

**Fix**: Either reject corrupted records on read (return error), or add a mechanism to repair/remove corrupted entries.

---

### 39. Settings Version Number Hardcoded and Confusing (App.tsx:48, settings/mod.rs:47)

**Problem**: In `syncSettingsToBackend`, the code sends `version: 5` (a magic number). In Rust backend, `ClickerSettings::default()` sets `version: 3`. These should be synchronized and documented. The version is never checked for migration purposes.

**Fix**: Define a single `SETTINGS_VERSION` constant in one place and use it consistently. Consider adding version checking for future migrations.

---

### 40. TelemetryConsent Decline Also Saves Consent (store.ts:265-267)

```typescript
export async function setTelemetryConsent(_accepted: boolean): Promise<void> {
  await store.set("telemetry_consented", true);  // Always saves `true` regardless of parameter
  await store.save();
}
```

**Problem**: `setTelemetryConsent(false)` still saves `telemetry_consented = true`. The parameter is prefixed with `_` to indicate it's unused, and the function always records consent as granted, even if the user declined. This is misleading.

**Fix**: Actually use the `_accepted` parameter or rename function to `recordTelemetryConsent` to clarify it only records that consent was asked, not whether it was accepted.

---

## Recommended Priority Order

1. **High**: Fix tab change logic bug (#1)
2. **High**: Remove status polling (#3)
3. **High**: Fix backend version type mismatch (#9)
4. **High**: Fix top-level await in store (#2)
5. **High**: Fix Rust telemetry on close uses default settings instead of actual (#29)
6. **High**: CumulativeStats interface mismatch (TypeScript expects `sessionsSent` but Rust doesn't provide it) (#22b)
7. **High**: **Mouse button gets stuck at high CPS + duty cycle** - ensure mouse-up events are always posted on stop (#43, #44)
8. **Medium**: Clean up commented code (#14)
9. **Medium**: Implement MacroPanel or hide tab (#13)
10. **Medium**: Add user-facing error handling (#19, #20)
11. **Medium**: Deduplicate cursor position code (#12)
12. **Medium**: Fix ToggleBtn useEffect (#17)
13. **Medium**: Fix corner failsafe detection logic (checks X<=TL && Y<=TL instead of X<=left && Y<=top) (#22c)
14. **Medium**: Stats `sent` field never used - dead code (#22d)
15. **Medium**: Implement window monitor work area clamping for multi-monitor support (#41)
16. **Low**: Extract magic numbers (#24)
17. **Low**: Memoize panel components (#26)
18. **Low**: Add error boundaries (#6)
19. **Low**: Add missing `sessionsSent` field population in Rust stats.rs (#22b)
20. **Low**: Fix `reset_stats` button comment states "Clear" but code is commented out (#22e)
21. **Low**: Consider adding graceful shutdown for clicker thread on app exit (#36)
22. **Low**: Investigate hotkey modifier matching behavior - add Strict toggle if needed (#42)
23. **Low**: Review RNG randomization implementation for speed variation (#47)
24. **Low**: Fix CSS px vs rem for accessibility/scaling (#41)

---

## Summary of New Issues Found (vs original analysis)

| # | Issue | Severity | Category |
|---|-------|----------|----------|
| 22b | CumulativeStats interface mismatch (sessionsSent missing) | High | Bug |
| 22c | Corner failsafe detection uses single threshold for both axes | Medium | Bug |
| 22d | Stats `sent` field is never used (dead code) | Low | Code Quality |
| 22e | Reset stats button commented out, unclear intent | Low | Code Quality |
| 36 | No graceful shutdown for clicker thread | Medium | Bug |
| 37 | Hotkey suppression timing edge cases | Low | Bug |
| 38 | Stats hash verification warns but doesn't reject corrupted records | Low | Bug |
| 39 | Settings version number inconsistent (5 vs 3) and hardcoded | Low | Code Quality |
| 40 | `setTelemetryConsent(false)` still saves true | High | Bug |
| 41 | Window cropping on smaller/scaled displays | Medium | Bug |
| 42 | Hotkey modifier matching - extra modifiers block valid hotkeys | Medium | Bug |
| 43 | Mouse button gets stuck at high CPS + duty cycle | High | Bug |
| 44 | Right click may get stuck when disabled shortly after activation | High | Bug |
| 45 | Border and corner stop not working as intended | Medium | Bug |
| 46 | Lag spikes at high CPS | Low | Bug |
| 47 | Click randomization not random enough | Low | Bug |
| 48 | Duty cycle button removed in Windows but still in macOS | Low | Feature Parity |
| 49 | Time limit hover card split in two | Low | UI Bug |
| 50 | Double click on top bar maximizing window | Low | Bug |

---

## Issues from Original Windows Version (applicable to macOS fork)

These bugs were fixed in the Windows version and should be investigated in the macOS fork:

### 41. Window Cropping on Smaller/Scaled Displays (PR #96)

**Problem**: Window was not clamped to active monitor work area. On smaller or scaled displays, the window content could be cropped off-screen. The Windows version fixed this by:
- Clamping app window to active monitor work area
- Flipping `.panel-area` overflow to `auto` so content scrolls instead of being clipped
- Changing `height: 100vh` → `height: 100%` with `min-height: 0`

**Applies to macOS**: Yes — macOS uses fixed 800x600 window with no monitor work area clamping. On Retina displays or multi-monitor setups, this could cause issues.

**Fix**: Implement monitor work area clamping and overflow scrolling in TitleBar/App layout.

---

### 42. Hotkey Modifier Matching - Extra Modifiers Block Valid Hotkeys (PR #96)

**Problem**: Exact modifier matching meant holding an extra modifier (e.g., holding Cmd while pressing Ctrl+Y) would block the hotkey from firing. Windows fixed this by switching to one-way matching — required modifiers must be held, but extra modifiers are ignored.

**Applies to macOS**: Unknown — the macOS hotkey handling in `src/hotkeys.ts` and `src-tauri/src/hotkeys.rs` may or may not have this issue.

**Fix**: Consider adding a "Strict Hotkey Modifiers" toggle in Settings (like Windows did).

---

### 43. Clicks Hold at End of Auto Click (Issue #114 - Open, Apr 18 2026)

**Problem** (from Windows v3.4.1): At high CPS (400+) with Duty Cycle enabled, sometimes the mouse button gets stuck down after the clicker stops. User reports blue box in click tester staying highlighted (indicating held button). This happens with Hold mode at 400 CPS, Duty Cycle at 45%.

**Applies to macOS**: Likely yes — the macOS version shares similar CGEvent mouse handling and has the same Duty Cycle feature. The issue is likely a race condition where a mouse-up event is not posted when the clicker stops, especially at high speeds.

**Affected scenarios**:
- High CPS (400+)
- Duty Cycle enabled (45%)
- Hold or Toggle mode
- Stop triggered while events are in flight

**Fix**: Ensure all mouse-down events have corresponding mouse-up events posted, even when stop is triggered mid-batch. Consider adding a flush/guarantee mechanism for pending mouse-up events.

---

### 44. Right Click May Get Stuck When Autoclicker Disabled Shortly After Activation (Issue #93)

**Problem**: "Right click may get stuck when autoclicker is disabled shortly after activation" — the mouse button stays held down after stop.

**Applies to macOS**: Possibly — relates to the same mouse event posting race condition as #43, but specifically for Right mouse button. The macOS version uses `CGEventType::RightMouseDown/Up` which could have similar issues.

**Fix**: Same as #43 — ensure mouse-up events are always posted on stop.

---

### 45. Border and Corner Stop Not Working as Intended - Visual Only (Issue #91)

**Problem**: Edge/Corner stop detection logic doesn't match the visual overlay representation.

**Applies to macOS**: Yes — macOS has the same corner/edge stop feature in `engine/failsafe.rs`. The detection logic issue noted in #22c (using single threshold for both axes) may be related.

**Fix**: Align the visual overlay with the actual failsafe detection logic.

---

### 46. Lag Spikes at High CPS (Issue #77)

**Problem**: Users report lag spikes at high click speeds.

**Applies to macOS**: Possibly — the macOS version polls for status every 200ms (#3) which could cause performance issues. Also, the batch event mechanism may not be optimally implemented.

**Fix**: Remove status polling (issue #3). Review batch event timing.

---

### 47. Click Randomization Not Random Enough (Issue #95)

**Problem**: "Click Randomisation not random enough" — speed variation may not be sufficiently random.

**Applies to macOS**: Yes — macOS has `speedVariation` in `worker.rs`. The RNG uses `next_gaussian()` for Gaussian distribution, which should be fine, but the implementation may need review.

**Fix**: Review RNG seeding and distribution implementation in `engine/rng.rs`.

---

### 48. Duty Cycle Button Removed in Windows v3.3.0 - Still Present in macOS

**Problem**: In Windows v3.3.0, the "Duty Cycle On/Off button" was removed. The macOS version still has `dutyCycleEnabled` toggle in the UI.

**Applies to macOS**: The UI still shows Duty Cycle On/Off buttons. This may be intentional (different design decisions), but worth noting.

---

### 49. Time Limit Hover Card Split in Two (v3.4.0 - Fixed)

**Problem**: "Fixed Time Limit hover card being split in 2" in v3.4.0.

**Applies to macOS**: Unknown — the macOS version may not have this specific UI issue, but similar hover card behavior may exist.

---

### 50. Double Click on Top Bar Maximizing Window (v3.3.0 - Fixed)

**Problem**: "Fixed double click on top bar maximizing the window" in v3.3.0.

**Applies to macOS**: The macOS version uses native window decorations (`decorations: true` in tauri.conf.json), so this should not apply. But custom title bar handling in `TitleBar.tsx` uses `WebkitAppRegion` which could have similar issues.

**Fix**: Ensure double-click on title bar doesn't trigger unintended window behaviors.

---

*Generated from codebase analysis. Last updated: 2026-04-18*