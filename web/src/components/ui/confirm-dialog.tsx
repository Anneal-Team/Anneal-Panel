import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";

type ConfirmDialogProps = {
  open: boolean;
  title: string;
  description: string;
  confirmLabel: string;
  pendingLabel?: string;
  isPending?: boolean;
  onClose: () => void;
  onConfirm: () => void;
};

export function ConfirmDialog({
  open,
  title,
  description,
  confirmLabel,
  pendingLabel,
  isPending,
  onClose,
  onConfirm,
}: ConfirmDialogProps) {
  const { t } = useTranslation();

  return (
    <Dialog open={open} onClose={onClose} title={title} description={description} className="max-w-xl">
      <div className="space-y-6">
        <div className="rounded-2xl bg-[#1d271a]/5 p-4 text-sm font-medium text-[#c43232]">
          {t("dialog.warning")}
        </div>
        <div className="flex justify-end gap-3">
          <Button type="button" variant="secondary" onClick={onClose} disabled={isPending}>
            {t("dialog.cancel")}
          </Button>
          <Button type="button" onClick={onConfirm} disabled={isPending}>
            {isPending ? pendingLabel ?? t("dialog.deleting") : confirmLabel}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
