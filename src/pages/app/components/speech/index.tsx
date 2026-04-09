import { useState, useCallback, useEffect } from "react";
import {
  Button,
  Popover,
  PopoverTrigger,
  PopoverContent,
} from "@/components";
import {
  HeadphonesIcon,
  AlertCircleIcon,
  LoaderIcon,
  AudioLinesIcon,
  CameraIcon,
  XIcon,
  SettingsIcon,
  PlusIcon,
} from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import { ResultsSection } from "./ResultsSection";
import { SettingsPanel } from "./SettingsPanel";
import { PermissionFlow } from "./PermissionFlow";
import { QuickActions } from "./QuickActions";
import { useSystemAudioType } from "@/hooks";
import { useApp } from "@/contexts";
import { cn } from "@/lib/utils";

export const SystemAudio = (props: useSystemAudioType) => {
  const {
    capturing,
    isProcessing,
    isAIProcessing,
    lastTranscription,
    lastAIResponse,
    error,
    setupRequired,
    startCapture,
    stopCapture,
    isPopoverOpen,
    setIsPopoverOpen,
    useSystemPrompt,
    setUseSystemPrompt,
    contextContent,
    setContextContent,
    startNewConversation,
    conversation,
    resizeWindow,
    quickActions,
    addQuickAction,
    removeQuickAction,
    isManagingQuickActions,
    setIsManagingQuickActions,
    showQuickActions,
    setShowQuickActions,
    handleQuickActionClick,
    scrollAreaRef,
    liveChunks,
    sentBoundary,
  } = props;

  const { supportsImages } = useApp();

  const [screenshotImage, setScreenshotImage] = useState<string | null>(null);
  const [isCapturingScreenshot, setIsCapturingScreenshot] = useState(false);
  const [showSettings, setShowSettings] = useState(false);

  const hasResponse = lastAIResponse || isAIProcessing;

  useEffect(() => {
    if (isProcessing && screenshotImage) {
      setScreenshotImage(null);
    }
  }, [isProcessing, screenshotImage]);

  const handleToggleCapture = async () => {
    if (capturing) {
      await stopCapture();
    } else {
      await startCapture();
    }
  };

  const handleCaptureScreenshot = useCallback(async () => {
    if (isCapturingScreenshot) return;

    setIsCapturingScreenshot(true);
    try {
      const platform = navigator.platform.toLowerCase();
      if (platform.includes("mac")) {
        const {
          checkScreenRecordingPermission,
          requestScreenRecordingPermission,
        } = await import("tauri-plugin-macos-permissions-api");

        const hasPermission = await checkScreenRecordingPermission();
        if (!hasPermission) {
          await requestScreenRecordingPermission();
          setIsCapturingScreenshot(false);
          return;
        }
      }

      const base64: string = await invoke("capture_screenshot", {
        screenId: null,
      });

      setScreenshotImage(base64);
    } catch (err) {
      console.error("Failed to capture screenshot:", err);
    } finally {
      setIsCapturingScreenshot(false);
    }
  }, [isCapturingScreenshot]);

  const handleRemoveScreenshot = useCallback(() => {
    setScreenshotImage(null);
  }, []);

  const getButtonIcon = () => {
    if (setupRequired) return <AlertCircleIcon className="text-orange-500" />;
    if (error && !setupRequired)
      return <AlertCircleIcon className="text-red-500" />;
    if (isProcessing) return <LoaderIcon className="animate-spin" />;
    if (capturing)
      return <AudioLinesIcon className="text-green-500 animate-pulse" />;
    return <HeadphonesIcon />;
  };

  const getButtonTitle = () => {
    if (setupRequired) return "Setup required - Click for instructions";
    if (error && !setupRequired) return `Error: ${error}`;
    if (isProcessing) return "Transcribing audio...";
    if (capturing) return "Stop system audio capture";
    return "Start system audio capture";
  };

  return (
    <Popover
      open={isPopoverOpen}
      onOpenChange={(open) => {
        if (capturing && !open) {
          return;
        }
        setIsPopoverOpen(open);
      }}
    >
      <PopoverTrigger asChild>
        <Button
          size="icon"
          title={getButtonTitle()}
          onClick={handleToggleCapture}
          className={cn(
            capturing && "bg-green-50 hover:bg-green-100",
            error && "bg-red-100 hover:bg-red-200"
          )}
        >
          {getButtonIcon()}
        </Button>
      </PopoverTrigger>

      {(capturing || setupRequired || error) && (
        <PopoverContent
          align="end"
          side="bottom"
          className="select-none w-screen p-0 border shadow-lg overflow-hidden border-input/50"
          sideOffset={8}
        >
          <div className="flex flex-col h-[calc(100vh-4rem)] overflow-hidden">
            {/* Header */}
            <div className="flex-shrink-0 px-3 py-2 border-b border-border/50 flex items-center justify-between gap-2">
              <div className="flex items-center gap-2">
                {capturing && (
                  <span className="flex items-center gap-1.5">
                    <span className="w-1.5 h-1.5 rounded-full bg-green-500 animate-pulse" />
                    <span className="text-xs font-medium">Live</span>
                  </span>
                )}
                {setupRequired && (
                  <h2 className="font-semibold text-sm">Setup Required</h2>
                )}
                {error && !setupRequired && (
                  <span className="text-[10px] text-red-600 truncate max-w-[200px]">
                    {error}
                  </span>
                )}
              </div>

              <div className="flex items-center gap-1 flex-shrink-0">
                {/* Screenshot */}
                {!setupRequired && supportsImages && (
                  <Button
                    size="icon"
                    variant={screenshotImage ? "default" : "ghost"}
                    onClick={handleCaptureScreenshot}
                    disabled={isCapturingScreenshot}
                    className="h-6 w-6"
                    title="Attach screenshot"
                  >
                    {isCapturingScreenshot ? (
                      <LoaderIcon className="w-3 h-3 animate-spin" />
                    ) : (
                      <CameraIcon className="w-3 h-3" />
                    )}
                  </Button>
                )}

                {/* New */}
                {!setupRequired && (
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={startNewConversation}
                    className="h-6 w-6"
                    title="New session"
                  >
                    <PlusIcon className="w-3 h-3" />
                  </Button>
                )}

                {/* Stop */}
                {capturing && (
                  <Button
                    size="icon"
                    variant="ghost"
                    onClick={stopCapture}
                    className="h-6 w-6 text-red-500 hover:text-red-600 hover:bg-red-50"
                    title="Stop capture"
                  >
                    <XIcon className="w-3 h-3" />
                  </Button>
                )}

                {/* Close */}
                {!capturing && (
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-6 w-6"
                    title="Close"
                    onClick={() => {
                      setIsPopoverOpen(false);
                      resizeWindow(false);
                    }}
                  >
                    <XIcon className="w-3 h-3" />
                  </Button>
                )}
              </div>
            </div>

            {/* Screenshot preview strip */}
            {screenshotImage && (
              <div className="flex-shrink-0 px-3 py-1.5 border-b border-border/50 bg-muted/30 flex items-center gap-2">
                <img
                  src={`data:image/png;base64,${screenshotImage}`}
                  alt="Screenshot"
                  className="h-8 w-14 object-cover rounded"
                />
                <span className="text-[9px] text-muted-foreground">
                  Screenshot attached
                </span>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-4 w-4 ml-auto"
                  onClick={handleRemoveScreenshot}
                >
                  <XIcon className="h-2.5 w-2.5" />
                </Button>
              </div>
            )}

            {/* Main content */}
            <div className="flex-1 min-h-0 overflow-hidden">
              {setupRequired ? (
                <div className="p-3">
                  <PermissionFlow
                    onPermissionGranted={() => {
                      startCapture();
                    }}
                    onPermissionDenied={() => {}}
                  />
                </div>
              ) : (
                <ResultsSection
                  lastTranscription={lastTranscription}
                  lastAIResponse={lastAIResponse}
                  isAIProcessing={isAIProcessing}
                  isProcessing={isProcessing}
                  capturing={capturing}
                  conversation={conversation}
                  liveChunks={liveChunks}
                  sentBoundary={sentBoundary}
                  scrollAreaRef={scrollAreaRef}
                />
              )}
            </div>

            {/* Footer: Quick actions + Settings toggle */}
            {!setupRequired && (
              <div className="flex-shrink-0 border-t border-border/50">
                {hasResponse && (
                  <div className="px-2 pt-1.5 pb-1">
                    <QuickActions
                      actions={quickActions}
                      onActionClick={handleQuickActionClick}
                      onAddAction={addQuickAction}
                      onRemoveAction={removeQuickAction}
                      isManaging={isManagingQuickActions}
                      setIsManaging={setIsManagingQuickActions}
                      show={showQuickActions}
                      setShow={setShowQuickActions}
                    />
                  </div>
                )}

                {/* Settings drawer */}
                <div className="border-t border-border/50">
                  <button
                    type="button"
                    onClick={() => setShowSettings(!showSettings)}
                    className="w-full flex items-center justify-between px-3 py-1.5 hover:bg-muted/50 transition-colors"
                  >
                    <div className="flex items-center gap-1.5">
                      <SettingsIcon className="w-3 h-3 text-muted-foreground" />
                      <span className="text-[10px] text-muted-foreground">
                        Settings
                      </span>
                    </div>
                    <svg
                      className={cn(
                        "w-3 h-3 text-muted-foreground transition-transform",
                        showSettings && "rotate-180"
                      )}
                      fill="none"
                      viewBox="0 0 24 24"
                      stroke="currentColor"
                    >
                      <path
                        strokeLinecap="round"
                        strokeLinejoin="round"
                        strokeWidth={2}
                        d="M19 9l-7 7-7-7"
                      />
                    </svg>
                  </button>
                  {showSettings && (
                    <div className="px-3 pb-2">
                      <SettingsPanel
                        useSystemPrompt={useSystemPrompt}
                        setUseSystemPrompt={setUseSystemPrompt}
                        contextContent={contextContent}
                        setContextContent={setContextContent}
                      />
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        </PopoverContent>
      )}
    </Popover>
  );
};
