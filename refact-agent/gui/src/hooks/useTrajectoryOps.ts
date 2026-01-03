import { useState, useCallback } from "react";
import { useAppDispatch, useAppSelector } from "./index";
import { selectChatId } from "../features/Chat";
import {
  usePreviewTransformMutation,
  useApplyTransformMutation,
  usePreviewHandoffMutation,
  useApplyHandoffMutation,
  TransformOptions,
  HandoffOptions,
  TransformPreviewResponse,
  HandoffPreviewResponse,
} from "../services/refact/trajectory";
import { createChatWithId, switchToThread } from "../features/Chat/Thread/actions";
import { push } from "../features/Pages/pagesSlice";

export type TrajectoryTab = "compress" | "handoff";

export function useTrajectoryOps() {
  const dispatch = useAppDispatch();
  const chatId = useAppSelector(selectChatId);

  const [activeTab, setActiveTab] = useState<TrajectoryTab>("compress");
  const [transformOptions, setTransformOptions] = useState<TransformOptions>({
    compress_attachments: true,
    compress_tool_results: true,
    summarize_conversation: false,
  });
  const [handoffOptions, setHandoffOptions] = useState<HandoffOptions>({
    include_summary: true,
    include_key_files: true,
    include_recent_context: true,
  });

  const [transformPreview, setTransformPreview] = useState<TransformPreviewResponse | null>(null);
  const [handoffPreview, setHandoffPreview] = useState<HandoffPreviewResponse | null>(null);

  const [previewTransform, { isLoading: isPreviewingTransform }] = usePreviewTransformMutation();
  const [applyTransform, { isLoading: isApplyingTransform }] = useApplyTransformMutation();
  const [previewHandoff, { isLoading: isPreviewingHandoff }] = usePreviewHandoffMutation();
  const [applyHandoff, { isLoading: isApplyingHandoff }] = useApplyHandoffMutation();

  const handlePreviewTransform = useCallback(async () => {
    if (!chatId) return;
    try {
      const result = await previewTransform({ chatId, options: transformOptions }).unwrap();
      setTransformPreview(result);
    } catch {
      setTransformPreview(null);
    }
  }, [chatId, transformOptions, previewTransform]);

  const handleApplyTransform = useCallback(async () => {
    if (!chatId) return false;
    try {
      const result = await applyTransform({ chatId, options: transformOptions }).unwrap();
      setTransformPreview(null);
      return result.success;
    } catch {
      return false;
    }
  }, [chatId, transformOptions, applyTransform]);

  const handlePreviewHandoff = useCallback(async () => {
    if (!chatId) return;
    try {
      const result = await previewHandoff({ chatId, options: handoffOptions }).unwrap();
      setHandoffPreview(result);
    } catch {
      setHandoffPreview(null);
    }
  }, [chatId, handoffOptions, previewHandoff]);

  const handleApplyHandoff = useCallback(async () => {
    if (!chatId) return false;
    try {
      const result = await applyHandoff({ chatId, options: handoffOptions }).unwrap();
      if (result.success && result.new_chat_id) {
        dispatch(createChatWithId({ id: result.new_chat_id }));
        dispatch(switchToThread({ id: result.new_chat_id }));
        dispatch(push({ name: "chat" }));
        setHandoffPreview(null);
        return true;
      }
      return false;
    } catch {
      return false;
    }
  }, [chatId, handoffOptions, applyHandoff, dispatch]);

  const clearPreviews = useCallback(() => {
    setTransformPreview(null);
    setHandoffPreview(null);
  }, []);

  const updateTransformOption = useCallback((key: keyof TransformOptions, value: boolean) => {
    setTransformOptions((prev) => ({ ...prev, [key]: value }));
    setTransformPreview(null);
  }, []);

  const updateHandoffOption = useCallback((key: keyof HandoffOptions, value: boolean) => {
    setHandoffOptions((prev) => ({ ...prev, [key]: value }));
    setHandoffPreview(null);
  }, []);

  return {
    chatId,
    activeTab,
    setActiveTab,
    transformOptions,
    handoffOptions,
    transformPreview,
    handoffPreview,
    isPreviewingTransform,
    isApplyingTransform,
    isPreviewingHandoff,
    isApplyingHandoff,
    handlePreviewTransform,
    handleApplyTransform,
    handlePreviewHandoff,
    handleApplyHandoff,
    clearPreviews,
    updateTransformOption,
    updateHandoffOption,
  };
}
