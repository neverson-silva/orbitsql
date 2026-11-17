import type { ReactNode } from "react";
import { cn } from "../../lib/utils";
import { Label } from "./label";

function PropertySection({
  children,
  className,
  title,
}: {
  children: ReactNode;
  className?: string;
  title: string;
}) {
  return (
    <section className={cn("space-y-2.5", className)}>
      <h2 className="font-mono text-label-caps font-semibold uppercase ds-tracking-caps-wide text-on-surface-variant">
        {title}
      </h2>
      <div className="space-y-1.5">{children}</div>
    </section>
  );
}

function PropertyRow({
  children,
  className,
  description,
  htmlFor,
  label,
}: {
  children: ReactNode;
  className?: string;
  description?: ReactNode;
  htmlFor: string;
  label: string;
}) {
  return (
    <div
      className={cn(
        "grid min-w-0 grid-cols-1 items-center gap-1.5 md:grid-cols-[7rem_minmax(0,1fr)] md:gap-x-3",
        className,
      )}
    >
      <Label
        className="text-label-caps font-medium text-on-surface-variant"
        htmlFor={htmlFor}
      >
        {label}
      </Label>
      <div className="min-w-0">{children}</div>
      {description ? (
        <p className="text-label-caps leading-code-sm text-on-surface-variant md:col-start-2">
          {description}
        </p>
      ) : null}
    </div>
  );
}

export { PropertyRow, PropertySection };
