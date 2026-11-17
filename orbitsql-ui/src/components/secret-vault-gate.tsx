import { useEffect, useState } from "react";
import {
  connectionErrorMessage,
  createSecretVault,
  secretStoreStatus,
  unlockSecretVault,
} from "../features/connections/connection-ipc";
import { Button } from "./ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "./ui/dialog";
import { Icon } from "./ui/icons";
import { Input } from "./ui/input";

type VaultMode = "create" | "unlock";

const MIN_MASTER_PASSWORD_LENGTH = 8;

function PasswordField({
  autoFocus = false,
  autoComplete,
  id,
  label,
  onChange,
  value,
}: {
  autoFocus?: boolean;
  autoComplete: string;
  id: string;
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  const [visible, setVisible] = useState(false);
  const [capsLockOn, setCapsLockOn] = useState(false);

  return (
    <label className="grid gap-1.5" htmlFor={id}>
      <span className="text-xs font-medium text-on-surface">{label}</span>
      <span className="relative">
        <Input
          aria-describedby={`${id}-hint`}
          autoComplete={autoComplete}
          autoFocus={autoFocus}
          className="pe-10"
          id={id}
          onBlur={() => setCapsLockOn(false)}
          onChange={(event) => onChange(event.target.value)}
          onKeyDown={(event) =>
            setCapsLockOn(event.getModifierState("CapsLock"))
          }
          type={visible ? "text" : "password"}
          value={value}
        />
        <Button
          aria-label={
            visible
              ? `Hide ${label.toLowerCase()}`
              : `Show ${label.toLowerCase()}`
          }
          className="absolute top-1/2 right-1 -translate-y-1/2 text-control-icon"
          onClick={() => setVisible((current) => !current)}
          size="icon-xs"
          type="button"
          variant="ghost"
        >
          <Icon name={visible ? "eye-off" : "eye"} size={15} />
        </Button>
      </span>
      <span className="sr-only" id={`${id}-hint`}>
        {capsLockOn ? "Caps Lock is on." : ""}
      </span>
      {capsLockOn ? (
        <span className="text-xs text-warning" role="status">
          Caps Lock is on
        </span>
      ) : null}
    </label>
  );
}

export function SecretVaultGate() {
  const isMacOS =
    navigator.platform.toLowerCase().startsWith("mac") ||
    navigator.userAgent.includes("Macintosh");
  const [mode, setMode] = useState<VaultMode | null>(null);
  const [masterPassword, setMasterPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const isCreating = mode === "create";
  const passwordTooShort =
    isCreating &&
    masterPassword.length > 0 &&
    masterPassword.length < MIN_MASTER_PASSWORD_LENGTH;
  const hasConfirmation = confirmation.length > 0;
  const passwordsMatch = masterPassword === confirmation;
  const canSubmit =
    !busy &&
    masterPassword.length > 0 &&
    (!isCreating ||
      (masterPassword.length >= MIN_MASTER_PASSWORD_LENGTH &&
        hasConfirmation &&
        passwordsMatch));

  useEffect(() => {
    if (isMacOS) return;
    let mounted = true;
    void secretStoreStatus()
      .then((status) => {
        if (!mounted || !status.startsWith("vault:")) return;
        setMode(status === "vault:uninitialized" ? "create" : "unlock");
      })
      .catch(() => undefined);
    return () => {
      mounted = false;
    };
  }, [isMacOS]);

  async function submit() {
    if (!canSubmit || !mode) return;
    if (isCreating && masterPassword !== confirmation) {
      setError("Master passwords do not match");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (isCreating) {
        await createSecretVault(masterPassword);
      } else {
        await unlockSecretVault(masterPassword);
      }
      setMasterPassword("");
      setConfirmation("");
      setMode(null);
    } catch (submitError) {
      setError(connectionErrorMessage(submitError));
    } finally {
      setBusy(false);
    }
  }

  if (isMacOS || !mode) return null;

  return (
    <Dialog open onOpenChange={() => undefined}>
      <DialogContent
        className="max-h-[calc(100vh-2rem)] w-[min(560px,calc(100vw-2rem))] max-w-none overflow-y-auto p-5 sm:max-w-none sm:p-6"
        showCloseButton={false}
      >
        <form
          className="grid gap-5"
          onSubmit={(event) => {
            event.preventDefault();
            void submit();
          }}
        >
          <DialogHeader className="gap-3 text-start">
            <div className="flex size-11 items-center justify-center rounded-md border border-primary/25 bg-primary/10 text-primary">
              <Icon name={isCreating ? "shield" : "lock"} size={24} />
            </div>
            <div className="grid gap-1.5">
              <DialogTitle className="text-lg tracking-tight">
                {isCreating
                  ? "Protect your credentials"
                  : "Unlock your credentials"}
              </DialogTitle>
              <DialogDescription className="max-w-[46ch] text-sm leading-relaxed">
                Orbit stores your saved database credentials in an encrypted
                vault on this device. Your master password unlocks that vault
                when Orbit needs access to them.
              </DialogDescription>
            </div>
          </DialogHeader>

          {isCreating ? (
            <div className="grid gap-3 rounded-md border border-outline-variant/60 bg-surface-container-highest/45 p-3.5">
              <div className="flex gap-3">
                <Icon
                  className="mt-0.5 shrink-0 text-primary"
                  name="shield"
                  size={18}
                />
                <div className="grid gap-0.5">
                  <p className="text-xs font-medium text-on-surface">
                    Local and encrypted
                  </p>
                  <p className="text-xs leading-relaxed text-on-surface-variant">
                    Your saved database credentials stay in Orbit&apos;s
                    encrypted local vault.
                  </p>
                  <p className="text-xs leading-relaxed text-on-surface-variant">
                    Credentials previously saved in the system credential store
                    must be entered again once to store them in this vault.
                  </p>
                </div>
              </div>
              <div className="flex gap-3">
                <Icon
                  className="mt-0.5 shrink-0 text-on-surface-variant"
                  name="key"
                  size={18}
                />
                <div className="grid gap-0.5">
                  <p className="text-xs font-medium text-on-surface">
                    Protected by your master password
                  </p>
                  <p className="text-xs leading-relaxed text-on-surface-variant">
                    Use it to unlock the vault when a saved credential is
                    needed.
                  </p>
                </div>
              </div>
            </div>
          ) : null}

          <section
            className="grid gap-3"
            aria-labelledby="vault-password-section"
          >
            <div className="grid gap-1">
              <h2
                className="text-sm font-medium text-on-surface"
                id="vault-password-section"
              >
                {isCreating
                  ? "Create your master password"
                  : "Enter your master password"}
              </h2>
              {isCreating ? (
                <p className="text-xs leading-relaxed text-on-surface-variant">
                  Use a long, unique password you do not use elsewhere.
                </p>
              ) : null}
            </div>
            <PasswordField
              autoFocus
              autoComplete={isCreating ? "new-password" : "current-password"}
              id="vault-master-password"
              label="Master password"
              onChange={setMasterPassword}
              value={masterPassword}
            />
            {isCreating ? (
              <div className="grid gap-1">
                <div className="flex items-center justify-between text-xs">
                  <span className="text-on-surface-variant">
                    Password length
                  </span>
                  <span
                    className={
                      passwordTooShort
                        ? "text-warning"
                        : "text-on-surface-variant"
                    }
                  >
                    {masterPassword.length}/{MIN_MASTER_PASSWORD_LENGTH} minimum
                  </span>
                </div>
                <div
                  className="h-1 overflow-hidden rounded-full bg-surface-container-highest"
                  aria-hidden="true"
                >
                  <div
                    className="h-full bg-primary transition-[width] duration-150"
                    style={{
                      width: `${Math.min((masterPassword.length / MIN_MASTER_PASSWORD_LENGTH) * 100, 100)}%`,
                    }}
                  />
                </div>
              </div>
            ) : null}
            {isCreating ? (
              <>
                <PasswordField
                  autoComplete="new-password"
                  id="vault-confirm-password"
                  label="Confirm master password"
                  onChange={setConfirmation}
                  value={confirmation}
                />
                {hasConfirmation ? (
                  <p
                    className={
                      passwordsMatch
                        ? "text-xs text-success"
                        : "text-xs text-destructive"
                    }
                    role={passwordsMatch ? "status" : "alert"}
                  >
                    {passwordsMatch
                      ? "Passwords match"
                      : "Passwords do not match"}
                  </p>
                ) : null}
              </>
            ) : null}
          </section>

          {isCreating ? (
            <div className="flex gap-3 rounded-md border border-warning/25 bg-warning/10 p-3 text-on-warning-container">
              <Icon className="mt-0.5 shrink-0" name="warning" size={18} />
              <div className="grid gap-0.5">
                <p className="text-xs font-medium">Keep this password safe</p>
                <p className="text-xs leading-relaxed opacity-85">
                  Orbit cannot recover or reset your master password if you
                  forget it.
                </p>
              </div>
            </div>
          ) : null}
          {error ? (
            <p
              className="rounded-md border border-destructive/25 bg-destructive/10 p-3 text-xs text-destructive"
              role="alert"
            >
              {error}
            </p>
          ) : null}
          <DialogFooter className="mt-0">
            <Button disabled={!canSubmit} size="lg" type="submit">
              {busy
                ? isCreating
                  ? "Creating vault…"
                  : "Unlocking…"
                : isCreating
                  ? "Create secure vault"
                  : "Unlock vault"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
