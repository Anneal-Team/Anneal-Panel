import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";

import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { LanguageSwitcher } from "@/components/language-switcher";
import { api } from "@/lib/api";

type AuthStep = "credentials" | "totp" | "totp_setup";

export function LoginPage() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [step, setStep] = useState<AuthStep>("credentials");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [totpCode, setTotpCode] = useState("");
  const [setupSecret, setSetupSecret] = useState("");
  const [setupUrl, setSetupUrl] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (api.readSession().accessToken) {
      void navigate({ to: "/" });
    }
  }, [navigate]);

  const loginMutation = useMutation({
    mutationFn: () => api.login({ email, password }),
    onSuccess: async (result) => {
      setError(null);
      if (result.status === "authenticated") {
        api.storeAuthenticatedSession(result.tokens);
        await navigate({ to: "/" });
        return;
      }
      api.storePreAuthToken(result.pre_auth_token);
      if (result.status === "totp_setup_required") {
        const setup = await api.beginTotpSetup();
        setSetupSecret(setup.secret);
        setSetupUrl(setup.otpauth_url);
        setStep("totp_setup");
        return;
      }
      setStep("totp");
    },
    onError: (mutationError) => {
      setError(mutationError.message);
    },
  });

  const verifyMutation = useMutation({
    mutationFn: () => api.verifyTotp(totpCode),
    onSuccess: async (tokens) => {
      setError(null);
      api.storeAuthenticatedSession(tokens);
      await navigate({ to: "/" });
    },
    onError: (mutationError) => {
      setError(mutationError.message);
    },
  });

  return (
    <div className="mx-auto flex min-h-[70vh] max-w-5xl items-center">
      <div className="grid w-full gap-6 lg:grid-cols-[1.1fr_0.9fr]">
        <Card className="bg-gradient-to-br from-accent to-[#1f5e63] text-white">
          <div className="text-xs uppercase tracking-[0.35em] text-white/70">Anneal</div>
          <h1 className="mt-4 text-4xl font-semibold">Control Plane</h1>
          <p className="mt-4 max-w-xl text-sm text-white/80">
            {t("login.slogan")}
          </p>
          <div className="mt-10 grid gap-4 md:grid-cols-3">
            <div className="rounded-3xl bg-white/10 p-4">
              <div className="text-xs uppercase tracking-[0.25em] text-white/60">Auth</div>
              <div className="mt-3 text-2xl font-semibold">SSO + TOTP</div>
            </div>
            <div className="rounded-3xl bg-white/10 p-4">
              <div className="text-xs uppercase tracking-[0.25em] text-white/60">Security</div>
              <div className="mt-3 text-2xl font-semibold">RBAC staff</div>
            </div>
            <div className="rounded-3xl bg-white/10 p-4">
              <div className="text-xs uppercase tracking-[0.25em] text-white/60">Cluster</div>
              <div className="mt-3 text-2xl font-semibold">Native agent</div>
            </div>
          </div>
        </Card>
        <Card className="flex w-full flex-col p-8">
          <div className="flex items-center justify-between">
            <div className="text-xs uppercase tracking-[0.3em] text-foreground/80">Access</div>
            <LanguageSwitcher />
          </div>
          <h2 className="mt-6 text-3xl font-semibold">{t("login.title")}</h2>
          <p className="mt-2 text-sm text-foreground/80">
            {t("login.subtitle")}
          </p>
          {step === "credentials" ? (
            <form
              className="mt-8 grid gap-4"
              onSubmit={(event) => {
                event.preventDefault();
                loginMutation.mutate();
              }}
            >
              <Input
                placeholder={t("login.email")}
                value={email}
                onChange={(event) => setEmail(event.target.value)}
              />
              <Input
                placeholder={t("login.password")}
                type="password"
                value={password}
                onChange={(event) => setPassword(event.target.value)}
              />
              {error ? <div className="text-sm text-danger">{error}</div> : null}
              <Button disabled={loginMutation.isPending}>
                {loginMutation.isPending ? t("login.loading") : t("login.button")}
              </Button>
            </form>
          ) : null}
          {step !== "credentials" ? (
            <form
              className="mt-8 grid gap-4"
              onSubmit={(event) => {
                event.preventDefault();
                verifyMutation.mutate();
              }}
            >
              {step === "totp_setup" ? (
                <div className="rounded-3xl bg-[#f2efe4] p-4 text-sm text-foreground/75">
                  <div className="font-semibold text-foreground">{t("login.totp_setup")}</div>
                  <div className="mt-3 break-all">{t("login.secret")}: {setupSecret}</div>
                  <div className="mt-2 break-all">{t("login.otp_url")}: {setupUrl}</div>
                </div>
              ) : null}
              <Input
                placeholder={t("login.totp_code")}
                value={totpCode}
                onChange={(event) => setTotpCode(event.target.value)}
              />
              {error ? <div className="text-sm text-danger">{error}</div> : null}
              <Button disabled={verifyMutation.isPending}>
                {verifyMutation.isPending ? t("login.totp_verifying") : t("login.totp_verify")}
              </Button>
              <Button
                type="button"
                variant="secondary"
                onClick={() => {
                  setStep("credentials");
                  setTotpCode("");
                  setError(null);
                  api.clearSession();
                }}
              >
                {t("login.return")}
              </Button>
            </form>
          ) : null}
        </Card>
      </div>
    </div>
  );
}
