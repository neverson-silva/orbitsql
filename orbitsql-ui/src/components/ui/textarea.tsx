import { cn } from "cn";
import type * as React from "react";
import { formControlClassName } from "@/components/ui/form-control";

function Textarea({ className, ...props }: React.ComponentProps<"textarea">) {
  return (
    <textarea
      data-slot="textarea"
      className={cn(
        `flex field-sizing-content min-h-16 w-full border px-2.5 py-2 text-xs ${formControlClassName} md:text-xs`,
        className,
      )}
      {...props}
    />
  );
}

export { Textarea };
