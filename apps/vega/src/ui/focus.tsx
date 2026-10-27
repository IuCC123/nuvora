import React, { createContext, forwardRef, useContext, useEffect, useImperativeHandle, useMemo, useRef, useState, type ReactNode } from 'react';
import { Animated, Pressable, View, type StyleProp, type ViewProps, type ViewStyle } from 'react-native';
import * as Kepler from '@amazon-devices/react-native-kepler';

/** Vega's focus calls, which its types don't declare. */
interface FocusManagerApi {
  focus(target: unknown): void;
}
export const FocusManager = (Kepler as unknown as { FocusManager: FocusManagerApi }).FocusManager;

/**
 * A region that remembers where focus was: entering it again lands there
 * (`autoFocus`), or on `destinations` when given. Vega's TVFocusGuideView.
 */
export const FocusGuide = (Kepler as unknown as { TVFocusGuideView: React.ComponentType<ViewProps & { autoFocus?: boolean; destinations?: unknown[]; trapFocusUp?: boolean; trapFocusDown?: boolean; trapFocusLeft?: boolean; trapFocusRight?: boolean }> })
  .TVFocusGuideView;

/** Focuses `target` (a host component ref) after this frame's layout. */
export function focusLater(target: unknown) {
  if (!target) return;
  setTimeout(() => {
    try {
      FocusManager.focus(target);
    } catch {
      // Gone before it could take focus.
    }
  }, 0);
}

/**
 * Per screen: whether it is the one showing, and the last thing focused in
 * it, so coming back to it puts focus there again.
 */
interface ScreenFocus {
  active: boolean;
  remember(target: unknown): void;
}
export const ScreenFocusContext = createContext<ScreenFocus>({ active: true, remember: () => undefined });

/** True while this screen is the one on top. */
export const useScreenActive = () => useContext(ScreenFocusContext).active;

/** Keeps a screen mounted under the ones above it, and refocuses it when it comes back. */
export function ScreenHost({ active, children }: { active: boolean; children: ReactNode }) {
  const last = useRef<unknown>(null);
  const focus = useMemo<ScreenFocus>(() => ({ active, remember: (t) => (last.current = t) }), [active]);
  const wasActive = useRef(active);
  useEffect(() => {
    if (active && !wasActive.current) focusLater(last.current);
    wasActive.current = active;
  }, [active]);
  return (
    <ScreenFocusContext.Provider value={focus}>
      <View style={[{ position: 'absolute', left: 0, top: 0, right: 0, bottom: 0 }, !active && { display: 'none' }]}>{children}</View>
    </ScreenFocusContext.Provider>
  );
}

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/**
 * A press this soon after something appears belongs to the key that made it
 * appear: Vega delivers OK's release to whatever has focus by then, so a menu
 * opened on press would otherwise pick its first row at once.
 */
const SETTLE_MS = 350;

export interface FocusableProps {
  onPress?: () => void;
  onLongPress?: () => void;
  onFocus?: () => void;
  onBlur?: () => void;
  /** Takes focus when it appears (and when this prop turns true). */
  preferred?: boolean;
  disabled?: boolean;
  /** Grows to this when focused, as the Fire TV app's cards do. */
  scale?: number;
  style?: StyleProp<ViewStyle> | ((focused: boolean) => StyleProp<ViewStyle>);
  children: ReactNode | ((focused: boolean) => ReactNode);
}

/** Anything the remote can land on and press. */
export const Focusable = forwardRef<View, FocusableProps>(function Focusable(props, ref) {
  const { onPress, onLongPress, onFocus, onBlur, preferred, disabled, scale = 1, style, children } = props;
  const [focused, setFocused] = useState(false);
  const screen = useContext(ScreenFocusContext);
  const self = useRef<View>(null);
  useImperativeHandle(ref, () => self.current as View);
  const grow = useRef(new Animated.Value(1)).current;
  const shownAt = useRef(Date.now()).current;
  const settled = () => Date.now() - shownAt >= SETTLE_MS;
  useEffect(() => {
    if (scale === 1) return;
    Animated.timing(grow, { toValue: focused ? scale : 1, duration: 150, useNativeDriver: true }).start();
  }, [focused, scale, grow]);
  const resolved = typeof style === 'function' ? style(focused) : style;
  return (
    <AnimatedPressable
      ref={self}
      onPress={disabled || !onPress ? undefined : () => (console.warn(`NVDBG press settled=${settled()}`), settled() && onPress())}
      onLongPress={disabled || !onLongPress ? undefined : () => settled() && onLongPress()}
      onFocus={() => {
        setFocused(true);
        screen.remember(self.current);
        onFocus?.();
      }}
      onBlur={() => {
        setFocused(false);
        onBlur?.();
      }}
      focusable={!disabled}
      hasTVPreferredFocus={preferred}
      style={[resolved, scale !== 1 && { transform: [{ scale: grow }] }]}>
      {typeof children === 'function' ? children(focused) : children}
    </AnimatedPressable>
  );
});
