import { Input as InputPrimitive } from "@base-ui/react/input";
import { cn } from "cn";
import type * as React from "react";
import { formControlClassName } from "@/components/ui/form-control";

function Input({ className, type, ...props }: React.ComponentProps<"input">) {
  return (
    <InputPrimitive
      type={type}
      data-slot="input"
      className={cn(
        `h-8 w-full min-w-0 border px-2.5 py-1 text-xs ${formControlClassName} file:inline-flex file:h-6 file:border-0 file:bg-transparent file:text-xs file:font-medium file:text-foreground md:text-xs`,
        className,
      )}
      {...props}
    />
  );
}

export { Input };
