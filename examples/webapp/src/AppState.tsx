import React, { createContext, useContext, useMemo, useState } from 'react';
import init_hydra, * as hydra from 'hydra-web';

interface AppState {
  client: hydra.Client | null;
}

const AppState = createContext<AppState | null>(null);

export const useAppState = () => useContext(AppState);

export const AppStateProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [appState, setAppState] = useState<AppState | null>(null);

  useMemo(() => { // use useMemo to ensure that the client is only initialized once
    init_hydra().then(async () => {
      console.log('init done');
      const newClient = hydra.Client.new();
      await newClient.ready();
      setAppState({ client: newClient });
    });
  }, []);

  return (
    <AppState.Provider value={appState}>
      {children}
    </AppState.Provider>
  );
};