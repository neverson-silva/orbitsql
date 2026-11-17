import { cn } from "@/lib/utils";
import { profileColorHex } from "./profile-style";

export function ProfileColorIndicator({
  profile,
  color,
  className,
}: {
  profile: string;
  color?: string | null;
  className?: string;
}) {
  return (
    <span
      aria-label={`${profile} profile color`}
      className={cn(
        "size-2 shrink-0 rounded-full ring-1 ring-black/20 dark:ring-white/25",
        className,
      )}
      role="img"
      style={{ backgroundColor: profileColorHex(color) }}
      title={`${profile} profile`}
    />
  );
}
