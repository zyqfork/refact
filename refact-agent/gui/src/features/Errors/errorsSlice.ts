import { createSlice, type PayloadAction } from "@reduxjs/toolkit";

export type ErrorSliceState = {
  message: string | null;
  isAuthError?: boolean;
};

const initialState: ErrorSliceState = { message: null };
export const errorSlice = createSlice({
  name: "error",
  initialState,
  reducers: {
    setError: (state, action: PayloadAction<string>) => {
      if (state.message) return;
      state.message = action.payload;
    },
    setIsAuthError: (state, action: PayloadAction<boolean>) => {
      state.isAuthError = action.payload;
    },
    clearError: (state, _action: PayloadAction) => {
      state.message = null;
    },
  },
  selectors: {
    getErrorMessage: (state) => state.message,
    getIsAuthError: (state) => state.isAuthError,
  },
});

export const { setError, setIsAuthError, clearError } = errorSlice.actions;
export const { getErrorMessage, getIsAuthError } = errorSlice.selectors;
