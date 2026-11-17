import { CaretDownIcon } from "@phosphor-icons/react";
import { cn } from "cn";
import type * as React from "react";
import {
  formControlClassName,
  formControlIconClassName,
} from "@/components/ui/form-control";

type NativeSelectProps = Omit<React.ComponentProps<"select">, "size"> & {
  size?: "sm" | "default";
};

function NativeSelect({
  className,
  size = "default",
  ...props
}: NativeSelectProps) {
  return (
    <div
      className={cn("group/native-select relative w-fit", className)}
      data-slot="native-select-wrapper"
      data-size={size}
    >
      <select
        data-slot="native-select"
        data-size={size}
        className={`h-8 w-full min-w-0 appearance-none border py-1 pr-8 pl-2.5 text-xs select-none selection:bg-primary selection:text-primary-foreground ${formControlClassName} data-[size=sm]:h-7 data-[size=sm]:py-0.5`}
        {...props}
      />
      <CaretDownIcon
        className={`pointer-events-none absolute top-1/2 right-2.5 size-4 -translate-y-1/2 select-none ${formControlIconClassName}`}
        aria-hidden="true"
        data-slot="native-select-icon"
      />
    </div>
  );
}

function NativeSelectOption({
  className,
  ...props
}: React.ComponentProps<"option">) {
  return (
    <option
      data-slot="native-select-option"
      className={cn("bg-floating-surface text-control-foreground", className)}
      {...props}
    />
  );
}

function NativeSelectOptGroup({
  className,
  ...props
}: React.ComponentProps<"optgroup">) {
  return (
    <optgroup
      data-slot="native-select-optgroup"
      className={cn("bg-floating-surface text-control-foreground", className)}
      {...props}
    />
  );
}

export { NativeSelect, NativeSelectOptGroup, NativeSelectOption };
