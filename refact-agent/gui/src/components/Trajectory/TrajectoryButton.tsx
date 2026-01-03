import React, { useState } from "react";
import { IconButton, Tooltip } from "@radix-ui/themes";
import { ArchiveIcon } from "@radix-ui/react-icons";
import { TrajectoryPopover } from "./TrajectoryPopover";

type TrajectoryButtonProps = {
  forceOpen?: boolean;
  onOpenChange?: (open: boolean) => void;
};

export const TrajectoryButton: React.FC<TrajectoryButtonProps> = ({
  forceOpen,
  onOpenChange,
}) => {
  const [internalOpen, setInternalOpen] = useState(false);
  const isControlled = forceOpen !== undefined;
  const open = isControlled ? forceOpen : internalOpen;

  const handleOpenChange = (newOpen: boolean) => {
    if (!isControlled) {
      setInternalOpen(newOpen);
    }
    onOpenChange?.(newOpen);
  };

  return (
    <TrajectoryPopover open={open} onOpenChange={handleOpenChange}>
      <Tooltip content="Trajectory: Compress or Handoff">
        <IconButton
          variant="ghost"
          size="1"
          data-testid="trajectory-button"
          aria-label="Open trajectory options"
        >
          <ArchiveIcon />
        </IconButton>
      </Tooltip>
    </TrajectoryPopover>
  );
};
