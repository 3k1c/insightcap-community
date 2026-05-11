# Uninstall and Data Choices

InsightCAP Community is designed so that uninstalling the application does not
delete user data by default.

The default uninstall behavior should remove the application program files only.
Local app data, encrypted knowledge data, and recovery-related material should
only be removed when the user explicitly chooses the related uninstall option.

## Uninstall Principles

InsightCAP uninstall choices follow these principles:

- Default uninstall should not destroy user data.
- Destructive choices must be opt-in.
- Encrypted knowledge data and local decryption keys are separate concerns.
- Users should understand recovery impact before clearing the local decryption
  key.
- A custom knowledge folder should not be deleted unless the user explicitly
  chooses permanent deletion.

## Optional Uninstall Choices

The Windows uninstaller may show these InsightCAP-specific options.

### Delete Local App Data

This option removes local app data for InsightCAP.

It may include:

- Settings.
- Cache files.
- Downloaded models.
- The default local knowledge base.

This option does not delete a user-selected custom knowledge folder.

### Clear This Windows Account's Local Decryption Key

This option clears the local decryption key stored for the current Windows user
account.

If the encrypted knowledge base is kept, reinstalling InsightCAP will require
the 24-word recovery phrase saved during first-time setup.

If the recovery phrase is unavailable, the encrypted knowledge base cannot be
recovered.

### Permanently Delete Custom Knowledge Folder

This option permanently deletes the user-selected custom knowledge folder shown
in the uninstaller.

This action cannot be undone.

Users should only select this option after confirming that the folder is no
longer needed or has been backed up separately.

## Strong Warning Case

If the user chooses to clear the local decryption key while keeping the encrypted
knowledge base, the uninstaller should show a strong warning.

The warning should explain:

- The encrypted knowledge base will remain on disk.
- The local decryption key will be removed.
- Reinstalling InsightCAP will require the 24-word recovery phrase.
- Without the recovery phrase, the encrypted knowledge base cannot be recovered.

## Recommended Default State

All optional uninstall checkboxes should be unchecked by default.

This matches the expected uninstall behavior for desktop applications: removing
the program should not automatically remove user data.

## Before Selecting Destructive Options

Before selecting destructive uninstall options, users should confirm that:

- Important knowledge data has been backed up if needed.
- The 24-word recovery phrase is saved in a secure place.
- The selected custom knowledge folder is the intended folder.
- The user understands that permanent deletion cannot be undone.

## Reinstallation

After reinstalling InsightCAP, existing encrypted knowledge data can only be
unlocked if the required local decryption key is still available or the user has
the correct 24-word recovery phrase.
