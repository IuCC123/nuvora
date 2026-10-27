import React, { useRef, useState, type ReactNode } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { Icon, type IconName } from '../icons';
import { useLive } from '../live';
import { useLoad } from '../load';
import { useNav, type SectionRoute } from '../nav';
import { useSignedIn } from '../state';
import { colors, type } from '../theme';
import { FocusGuide, Focusable } from '../ui';

/** Which part of the app is showing, for the menu's highlight: `"home"`, `"library:3"`. */
export type Section = 'home' | 'search' | 'together' | 'settings' | `library:${number}`;

const COLLAPSED = 80;
const OPEN = 236;

/**
 * The side menu every main screen sits in, as on the Fire TV app: icons
 * when collapsed, labels when focus moves into it (Left from the content),
 * landing on the page you are on.
 */
export function Shell({ section, children }: { section: Section; children: ReactNode }) {
  const { client, live } = useSignedIn();
  const nav = useNav();
  const [libraries] = useLoad('libraries', () => client.libraries());
  const party = useLive(live);
  const list = libraries.kind === 'ready' ? libraries.value : [];
  const [focusedItem, setFocusedItem] = useState<Section | null>(null);
  const [current, setCurrent] = useState<View | null>(null);
  const closing = useRef<ReturnType<typeof setTimeout> | null>(null);
  const open = focusedItem !== null;

  const go = (to: Section, route: SectionRoute) => {
    if (to !== section) nav.section(route);
  };

  const item = (key: Section, icon: IconName, label: string, route: SectionRoute, dot = false) => (
    <Focusable
      key={key}
      ref={key === section ? setCurrent : undefined}
      onPress={() => go(key, route)}
      onFocus={() => {
        if (closing.current) clearTimeout(closing.current);
        setFocusedItem(key);
      }}
      onBlur={() => {
        // Moving between items blurs one before focusing the next: close only when focus has left.
        closing.current = setTimeout(() => setFocusedItem((f) => (f === key ? null : f)), 30);
      }}
      style={(focused) => [styles.item, key === section && styles.itemSelected, focused && styles.itemFocused]}>
      {(focused) => {
        const color = focused ? colors.canvas : key === section ? colors.accent : colors.secondary;
        return (
          <>
            <View>
              <Icon name={icon} size={22} color={color} />
              {dot && <View style={styles.dot} />}
            </View>
            {open && (
              <Text style={[styles.itemLabel, { color }]} numberOfLines={1}>
                {label}
              </Text>
            )}
          </>
        );
      }}
    </Focusable>
  );

  return (
    <View style={styles.screen}>
      <View style={styles.content}>{children}</View>
      {open && <View style={styles.scrim} pointerEvents="none" />}
      <FocusGuide destinations={current ? [current] : []} style={[styles.rail, { width: open ? OPEN : COLLAPSED }]}>
        <View style={styles.logo}>
          <Icon name="play" size={22} color={colors.accent} />
          {open && <Text style={styles.wordmark}>nuvora</Text>}
        </View>
        {item('search', 'search', 'Search', { name: 'search' })}
        {item('home', 'home', 'Home', { name: 'home' })}
        {item('together', 'users', party.party ? 'Watching together' : 'Watch together', { name: 'together' }, party.invites.length > 0 || party.party !== null)}
        {list.map((library) =>
          item(`library:${library.id}`, library.kind === 'shows' ? 'tv' : 'film', library.name, { name: 'library', id: library.id, title: library.name }),
        )}
        <View style={styles.spacer} />
        {item('settings', 'settings', 'Settings', { name: 'settings' })}
      </FocusGuide>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.canvas },
  content: { position: 'absolute', left: COLLAPSED, top: 0, right: 0, bottom: 0 },
  scrim: { ...StyleSheet.absoluteFillObject, backgroundColor: 'rgba(0,0,0,0.5)' },
  rail: { position: 'absolute', left: 0, top: 0, bottom: 0, backgroundColor: colors.canvas, paddingHorizontal: 12, paddingVertical: 28, gap: 4 },
  logo: { flexDirection: 'row', alignItems: 'center', height: 32, paddingLeft: 17, marginBottom: 22, gap: 8 },
  wordmark: { fontSize: 24, fontWeight: '700', color: colors.text },
  item: { flexDirection: 'row', alignItems: 'center', height: 48, paddingLeft: 17, borderRadius: 24, gap: 14 },
  itemSelected: { backgroundColor: colors.raised },
  itemFocused: { backgroundColor: colors.text },
  itemLabel: { ...type.titleSmall, flex: 1, paddingRight: 12 },
  dot: { position: 'absolute', top: -2, right: -3, width: 9, height: 9, borderRadius: 5, backgroundColor: colors.accent },
  spacer: { flex: 1 },
});
