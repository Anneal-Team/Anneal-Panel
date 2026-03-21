import { Link } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";

import { Card } from "@/components/ui/card";

export function AuthRequired({ title }: { title: string }) {
  const { t } = useTranslation();

  return (
    <div className="mx-auto flex min-h-[60vh] max-w-2xl items-center">
      <Card className="flex w-full flex-col p-8">
        <h1 className="mt-6 text-3xl font-semibold">{title}</h1>
        <p className="mt-2 mb-6 max-w-xl text-sm text-foreground/90">
          {t("auth_required.subtitle")}
        </p>
        <Link
          to="/login"
          className="inline-flex max-w-max rounded-2xl bg-[#a4d872] px-6 py-3 text-sm font-semibold text-[#1d271a] transition hover:bg-[#b5e983]"
        >
          {t("auth_required.button")}
        </Link>
      </Card>
    </div>
  );
}
