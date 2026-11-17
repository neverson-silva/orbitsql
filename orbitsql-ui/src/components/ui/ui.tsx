import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button as BaseButton } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { cn } from "@/lib/utils";
import { Icon, type IconName } from "./icons";

export { cn } from "@/lib/utils";

export function StatusPill({
  children,
  tone = "neutral",
  icon,
  className,
}: {
  children: ReactNode;
  tone?: "neutral" | "primary" | "success" | "warning" | "error" | "violet";
  icon?: IconName;
  className?: string;
}) {
  const tones = {
    neutral: "bg-surface-subtle text-on-surface-variant",
    primary: "border-primary/30 bg-primary-container text-on-primary-container",
    success: "border-success/30 bg-success-container text-on-success-container",
    warning: "border-warning/30 bg-warning-container text-on-warning-container",
    error: "border-error/30 bg-error-container text-on-error-container",
    violet:
      "border-secondary/30 bg-secondary-container text-on-secondary-container",
  } as const;
  return (
    <Badge
      className={cn(
        "gap-1 rounded-full px-2 py-1 font-mono text-label-caps font-medium leading-none",
        tones[tone],
        className,
      )}
    >
      {icon ? <Icon name={icon} size={12} /> : null}
      {children}
    </Badge>
  );
}

export function IconButton({
  label,
  name,
  onClick,
  active = false,
  size = "sm",
  className,
}: {
  label: string;
  name: IconName;
  onClick?: () => void;
  active?: boolean;
  size?: "sm" | "md";
  className?: string;
}) {
  return (
    <BaseButton
      aria-label={label}
      className={cn(
        size === "sm" ? "h-7 w-7" : "h-8 w-8",
        active &&
          "border-control-border-focus bg-selected-surface text-primary",
        className,
      )}
      onClick={onClick}
      size={size === "sm" ? "icon-sm" : "icon"}
      variant="ghost"
    >
      <Icon name={name} size={size === "sm" ? 14 : 16} />
    </BaseButton>
  );
}

export function Surface({
  children,
  className,
  tone = "raised",
}: {
  children: ReactNode;
  className?: string;
  tone?: "raised" | "subtle" | "glass";
}) {
  const tones = {
    raised: "bg-surface-raised",
    subtle: "bg-surface-subtle",
    glass: "bg-surface-raised/90 backdrop-blur-xl",
  } as const;
  return (
    <Card
      className={cn(
        "rounded-md border-surface-divider",
        tones[tone],
        className,
      )}
    >
      {children}
    </Card>
  );
}

export function ScreenHeader({
  eyebrow,
  title,
  description,
  actions,
}: {
  eyebrow?: string;
  title: string;
  description?: string;
  actions?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div>
        {eyebrow ? (
          <p className="mb-1 font-mono text-label-caps font-semibold uppercase ds-tracking-caps-wide text-primary">
            {eyebrow}
          </p>
        ) : null}
        <h1 className="text-headline-md font-semibold ds-tracking-headline text-on-surface">
          {title}
        </h1>
        {description ? (
          <p className="mt-1 max-w-2xl text-body-sm text-on-surface-variant">
            {description}
          </p>
        ) : null}
      </div>
      {actions ? (
        <div className="flex items-center gap-2">{actions}</div>
      ) : null}
    </div>
  );
}

export function Button({
  children,
  variant = "secondary",
  icon,
  onClick,
  type = "button",
  className,
  disabled = false,
  role,
  ariaChecked,
}: {
  children: ReactNode;
  variant?: "primary" | "secondary" | "ghost" | "danger";
  icon?: IconName;
  onClick?: () => void;
  type?: "button" | "submit" | "reset";
  className?: string;
  disabled?: boolean;
  role?: "radio";
  ariaChecked?: boolean;
}) {
  const variants = {
    primary:
      "border-primary/20 bg-primary-container text-on-primary hover:bg-primary",
    secondary:
      "border-control-border bg-control-surface text-control-foreground hover:border-control-border-hover hover:bg-control-surface-hover",
    ghost:
      "border-transparent bg-transparent text-on-surface-variant hover:bg-control-surface-hover hover:text-on-surface",
    danger: "border-error/20 bg-error/10 text-error hover:bg-error/15",
  } as const;
  const primitiveVariant = {
    primary: "default",
    secondary: "outline",
    ghost: "ghost",
    danger: "destructive",
  } as const;
  return (
    <BaseButton
      className={cn("text-body-sm", variants[variant], className)}
      disabled={disabled}
      onClick={onClick}
      aria-checked={ariaChecked}
      role={role}
      type={type}
      variant={primitiveVariant[variant]}
    >
      {icon ? <Icon name={icon} size={14} /> : null}
      {children}
    </BaseButton>
  );
}

export function MetricCard({
  label,
  value,
  detail,
  icon,
  tone = "primary",
}: {
  label: string;
  value: string;
  detail: string;
  icon: IconName;
  tone?: "primary" | "success" | "warning" | "violet" | "error";
}) {
  const colors = {
    primary: "text-primary bg-primary/10",
    success: "text-success bg-success/10",
    warning: "text-warning bg-warning/10",
    violet: "text-secondary bg-secondary/10",
    error: "text-error bg-error/10",
  } as const;
  return (
    <Surface className="flex min-h-28 flex-1 flex-col justify-between p-4">
      <div className="flex items-center justify-between gap-3">
        <span className="text-label-md font-medium text-on-surface-variant">
          {label}
        </span>
        <span
          className={cn(
            "inline-flex h-7 w-7 items-center justify-center rounded-md",
            colors[tone],
          )}
        >
          <Icon name={icon} size={15} />
        </span>
      </div>
      <div className="mt-3 flex items-end justify-between gap-2">
        <span className="font-mono text-headline-md font-semibold ds-tracking-headline text-on-surface">
          {value}
        </span>
        <span className="font-mono text-label-caps text-on-surface-variant">
          {detail}
        </span>
      </div>
    </Surface>
  );
}

export function SegmentedTabs({
  options,
  value,
  onChange,
  size = "sm",
  className,
}: {
  options: Array<{
    id: string;
    label: string;
    count?: number | string;
    icon?: IconName;
  }>;
  value: string;
  onChange: (value: string) => void;
  size?: "sm" | "md";
  className?: string;
}) {
  return (
    <Tabs
      className={cn("min-w-0 max-w-full", className)}
      value={value}
      onValueChange={onChange}
    >
      <TabsList className="no-scrollbar w-max max-w-full flex-nowrap gap-0 overflow-x-auto overflow-y-hidden rounded-md border border-control-border bg-control-surface p-0.5">
        {options.map((option) => (
          <TabsTrigger
            className={cn(
              "min-w-0 flex-none rounded-sm border-transparent px-2 text-label-caps text-on-surface-variant hover:bg-control-surface-hover hover:text-on-surface data-active:border-control-border-focus data-active:bg-selected-surface data-active:text-on-surface",
              size === "sm" ? "h-7" : "h-8",
            )}
            key={option.id}
            value={option.id}
          >
            {option.icon ? <Icon name={option.icon} size={13} /> : null}
            <span className="truncate">{option.label}</span>
            {option.count !== undefined ? (
              <span className="font-mono text-label-caps tabular-nums text-on-surface-variant/80">
                {option.count}
              </span>
            ) : null}
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}

export function EmptyState({
  title,
  detail,
  icon = "database",
}: {
  title: string;
  detail: string;
  icon?: IconName;
}) {
  return (
    <Empty className="min-h-48 rounded-md border-dashed border-surface-divider bg-transparent p-6 text-center">
      <EmptyHeader>
        <EmptyMedia
          className="mb-3 rounded-md bg-selected-surface text-primary"
          variant="icon"
        >
          <Icon name={icon} size={20} />
        </EmptyMedia>
        <EmptyTitle className="text-body-md font-semibold text-on-surface">
          {title}
        </EmptyTitle>
        <EmptyDescription className="mt-1 max-w-sm text-body-sm text-on-surface-variant">
          {detail}
        </EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
