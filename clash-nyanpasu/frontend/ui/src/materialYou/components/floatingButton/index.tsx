import { ReactNode } from "react";
import { alpha, Button, ButtonProps, useTheme } from "@mui/material";

export interface FloatingButtonProps extends ButtonProps {
  children: ReactNode;
  className?: string;
}

export const FloatingButton = ({
  children,
  className,
  ...props
}: FloatingButtonProps) => {
  const { palette } = useTheme();

  return (
    <Button
      className={`bottom-8 right-8 z-10 size-16 !rounded-2xl backdrop-blur ${className}`}
      sx={{
        position: "fixed",
        boxShadow: 8,
        backgroundColor: alpha(palette.primary.main, 0.3),

        "&:hover": {
          backgroundColor: alpha(palette.primary.main, 0.45),
        },
      }}
      {...props}
    >
      {children}
    </Button>
  );
};
