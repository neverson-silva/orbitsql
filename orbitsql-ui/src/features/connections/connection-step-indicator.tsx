type ConnectionStep = 1 | 2 | 3;

const steps = [
  { id: 1, label: "1 Engine" },
  { id: 2, label: "2 Configure" },
  { id: 3, label: "3 Review" },
] as const;

export function ConnectionStepIndicator({
  activeStep,
}: {
  activeStep: ConnectionStep;
}) {
  return (
    <ol
      aria-label="Connection setup progress"
      className="m-0 hidden list-none items-center gap-1.5 p-0 text-label-caps md:flex"
    >
      {steps.map((step) => (
        <li
          aria-current={activeStep === step.id ? "step" : undefined}
          className={
            activeStep === step.id
              ? "rounded-full bg-primary px-2.5 py-1 font-medium text-on-primary"
              : "px-1 text-on-surface-variant"
          }
          key={step.id}
        >
          {step.label}
        </li>
      ))}
    </ol>
  );
}
