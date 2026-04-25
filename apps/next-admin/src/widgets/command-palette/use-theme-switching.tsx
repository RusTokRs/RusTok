import { useRegisterActions } from 'kbar';
import { useTheme } from 'next-themes';
import { useEffectEvent, useMemo } from 'react';

const useThemeSwitching = () => {
  const { theme, setTheme } = useTheme();

  const toggleTheme = useEffectEvent(() => {
    setTheme(theme === 'light' ? 'dark' : 'light');
  });

  const setLightTheme = useEffectEvent(() => {
    setTheme('light');
  });

  const setDarkTheme = useEffectEvent(() => {
    setTheme('dark');
  });

  const themeAction = useMemo(
    () => [
      {
        id: 'toggleTheme',
        name: 'Toggle Theme',
        shortcut: ['t', 't'],
        section: 'Theme',
        perform: toggleTheme
      },
      {
        id: 'setLightTheme',
        name: 'Set Light Theme',
        section: 'Theme',
        perform: setLightTheme
      },
      {
        id: 'setDarkTheme',
        name: 'Set Dark Theme',
        section: 'Theme',
        perform: setDarkTheme
      }
    ],
    [setDarkTheme, setLightTheme, toggleTheme]
  );

  useRegisterActions(themeAction, [themeAction]);
};

export default useThemeSwitching;
