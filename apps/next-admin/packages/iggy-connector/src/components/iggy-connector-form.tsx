"use client";

import { useState, useTransition } from "react";
import { Button } from "@/shared/ui/shadcn/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/shared/ui/shadcn/card";
import {
  type IggyConnectorConfiguration,
  saveIggyConnectorConfiguration,
} from "../api/iggy-connector";

interface IggyConnectorFormProps {
  configuration: IggyConnectorConfiguration;
  token: string | null;
  tenantSlug: string | null;
}

function parseAddresses(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((address) => address.trim())
    .filter(Boolean);
}

export function IggyConnectorForm({
  configuration,
  token,
  tenantSlug,
}: IggyConnectorFormProps) {
  const [isPending, startTransition] = useTransition();
  const [mode, setMode] = useState(configuration.desiredMode);
  const [addresses, setAddresses] = useState(
    configuration.externalAddresses.join("\n"),
  );
  const [username, setUsername] = useState(configuration.externalUsername);
  const [resolver, setResolver] = useState(
    configuration.passwordResolver === "deployment"
      ? "env"
      : configuration.passwordResolver,
  );
  const [passwordKey, setPasswordKey] = useState(
    configuration.passwordResolver === "deployment"
      ? ""
      : configuration.passwordKey,
  );
  const [tlsEnabled, setTlsEnabled] = useState(configuration.tlsEnabled);
  const [tlsDomain, setTlsDomain] = useState(configuration.tlsDomain ?? "");
  const [result, setResult] = useState<{ ok: boolean; message: string } | null>(
    null,
  );

  function save() {
    startTransition(async () => {
      try {
        const outcome = await saveIggyConnectorConfiguration(
          {
            mode,
            externalAddresses: parseAddresses(addresses),
            externalUsername: username,
            passwordResolver: resolver,
            passwordKey,
            tlsEnabled,
            tlsDomain: tlsDomain.trim() || null,
          },
          { token, tenantSlug },
        );
        setResult({
          ok: true,
          message: outcome.restartRequired
            ? "Saved. Restart the server to activate this connector mode."
            : "Saved. This connector mode is already active.",
        });
      } catch (error) {
        setResult({ ok: false, message: String(error) });
      }
    });
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>Connection mode</CardTitle>
        </CardHeader>
        <CardContent className="space-y-5">
          <div className="grid gap-3 md:grid-cols-2">
            <button
              type="button"
              disabled={!configuration.bundledAvailable}
              onClick={() => setMode("bundled")}
              className={`rounded-lg border p-4 text-left disabled:cursor-not-allowed disabled:opacity-50 ${
                mode === "bundled" ? "border-primary bg-primary/5" : ""
              }`}
            >
              <p className="font-medium">Bundled</p>
              <p className="text-muted-foreground mt-1 text-sm">
                Use the Iggy artifact installed with this module.
              </p>
            </button>
            <button
              type="button"
              onClick={() => setMode("external")}
              className={`rounded-lg border p-4 text-left ${
                mode === "external" ? "border-primary bg-primary/5" : ""
              }`}
            >
              <p className="font-medium">External</p>
              <p className="text-muted-foreground mt-1 text-sm">
                Connect to an operator-managed Iggy deployment.
              </p>
            </button>
          </div>

          <dl className="grid max-w-xl grid-cols-2 gap-2 text-sm">
            <dt className="text-muted-foreground">Active mode</dt>
            <dd className="font-mono">{configuration.activeMode}</dd>
            <dt className="text-muted-foreground">Desired mode</dt>
            <dd className="font-mono">{configuration.desiredMode}</dd>
            <dt className="text-muted-foreground">Readiness</dt>
            <dd>
              {configuration.configured ? "Ready" : "Configuration required"}
            </dd>
          </dl>

          {configuration.configurationError && (
            <p className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-800 dark:text-amber-300">
              {configuration.configurationError}
            </p>
          )}
        </CardContent>
      </Card>

      {mode === "external" && (
        <Card>
          <CardHeader>
            <CardTitle>External Iggy</CardTitle>
          </CardHeader>
          <CardContent className="grid gap-5 md:grid-cols-2">
            <label className="space-y-2 md:col-span-2">
              <span className="text-sm font-medium">Iggy addresses</span>
              <textarea
                value={addresses}
                onChange={(event) => setAddresses(event.target.value)}
                placeholder="iggy.example.com:8090"
                className="min-h-24 w-full rounded-md border bg-transparent px-3 py-2 font-mono text-sm"
              />
              <span className="text-muted-foreground block text-xs">
                One host:port per line.
              </span>
            </label>
            <label className="space-y-2">
              <span className="text-sm font-medium">Username</span>
              <input
                value={username}
                onChange={(event) => setUsername(event.target.value)}
                className="w-full rounded-md border bg-transparent px-3 py-2 text-sm"
              />
            </label>
            <label className="space-y-2">
              <span className="text-sm font-medium">Secret resolver</span>
              <select
                value={resolver}
                onChange={(event) => setResolver(event.target.value)}
                className="w-full rounded-md border bg-background px-3 py-2 text-sm"
              >
                <option value="env">Environment variable</option>
                <option value="mounted_file">Mounted file</option>
              </select>
            </label>
            <label className="space-y-2 md:col-span-2">
              <span className="text-sm font-medium">Password secret key</span>
              <input
                value={passwordKey}
                onChange={(event) => setPasswordKey(event.target.value)}
                placeholder="RUSTOK_IGGY_PASSWORD"
                className="w-full rounded-md border bg-transparent px-3 py-2 font-mono text-sm"
              />
              <span className="text-muted-foreground block text-xs">
                Only the reference is stored. The password never enters the
                database or UI.
              </span>
            </label>
            <label className="flex items-center gap-3">
              <input
                type="checkbox"
                checked={tlsEnabled}
                onChange={(event) => setTlsEnabled(event.target.checked)}
              />
              <span className="text-sm font-medium">Enable TLS</span>
            </label>
            {tlsEnabled && (
              <label className="space-y-2">
                <span className="text-sm font-medium">
                  TLS server name (optional)
                </span>
                <input
                  value={tlsDomain}
                  onChange={(event) => setTlsDomain(event.target.value)}
                  className="w-full rounded-md border bg-transparent px-3 py-2 text-sm"
                />
              </label>
            )}
          </CardContent>
        </Card>
      )}

      <div className="flex items-center gap-4">
        <Button onClick={save} disabled={isPending}>
          {isPending ? "Saving..." : "Save connector settings"}
        </Button>
        {result && (
          <span
            className={
              result.ok ? "text-sm text-green-600" : "text-destructive text-sm"
            }
          >
            {result.message}
          </span>
        )}
      </div>
    </div>
  );
}
