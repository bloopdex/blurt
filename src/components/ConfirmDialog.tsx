import { create } from "zustand";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";

interface Request {
  title: string;
  description: string;
  confirmLabel: string;
  destructive?: boolean;
  resolve: (ok: boolean) => void;
}

const useConfirm = create<{ req: Request | null; set: (r: Request | null) => void }>((set) => ({
  req: null,
  set: (req) => set({ req }),
}));

/** Imperative confirm: `if (await confirm({...})) doIt()`. */
export function confirm(opts: Omit<Request, "resolve">): Promise<boolean> {
  return new Promise((resolve) => useConfirm.getState().set({ ...opts, resolve }));
}

export function ConfirmHost() {
  const { req, set } = useConfirm();
  const close = (ok: boolean) => {
    req?.resolve(ok);
    set(null);
  };
  return (
    <AlertDialog open={!!req} onOpenChange={(o) => !o && close(false)}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{req?.title}</AlertDialogTitle>
          <AlertDialogDescription>{req?.description}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel onClick={() => close(false)}>Cancel</AlertDialogCancel>
          <AlertDialogAction
            className={req?.destructive ? "bg-destructive text-white hover:bg-destructive/90" : undefined}
            onClick={() => close(true)}
          >
            {req?.confirmLabel}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
