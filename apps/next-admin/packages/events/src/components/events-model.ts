export interface EvaluateProfileSelectionOptions {
  activeProfile: string;
  iggyConfigured: boolean;
  serverRestartRequired: boolean;
}

export type ProfileSelectionResult =
  | {
      canApply: false;
      showIggyConfiguration: true;
      selectedProfile: string;
      restartRequired: boolean;
    }
  | {
      canApply: true;
      showIggyConfiguration: false;
      selectedProfile: string;
      restartRequired: boolean;
    };

export function isRestartRequired(
  selectedProfile: string,
  activeProfile: string,
  serverRestartRequired: boolean
): boolean {
  return serverRestartRequired || selectedProfile !== activeProfile;
}

export function evaluateProfileSelection(
  nextProfile: string,
  options: EvaluateProfileSelectionOptions
): ProfileSelectionResult {
  if (nextProfile === 'outbox_iggy' && !options.iggyConfigured) {
    return {
      canApply: false,
      showIggyConfiguration: true,
      selectedProfile: nextProfile,
      restartRequired: isRestartRequired(
        nextProfile,
        options.activeProfile,
        options.serverRestartRequired
      )
    };
  }

  return {
    canApply: true,
    showIggyConfiguration: false,
    selectedProfile: nextProfile,
    restartRequired: isRestartRequired(
      nextProfile,
      options.activeProfile,
      options.serverRestartRequired
    )
  };
}

export function formatSaveOutcomeKey(
  restartRequired: boolean
): 'saved' | 'savedWithoutRestart' {
  return restartRequired ? 'saved' : 'savedWithoutRestart';
}
