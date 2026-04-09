import { LiveChunk } from "@/hooks/useSystemAudio";
import { ChatConversation } from "@/types";
import { Markdown, CopyButton } from "@/components";
import { BotIcon, Loader2, HeadphonesIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { useEffect, useState, useRef, RefObject } from "react";

type Props = {
  lastTranscription: string;
  lastAIResponse: string;
  isAIProcessing: boolean;
  isProcessing: boolean;
  capturing: boolean;
  conversation: ChatConversation;
  liveChunks: LiveChunk[];
  sentBoundary: number;
  scrollAreaRef: RefObject<HTMLDivElement | null>;
};

function TypingText({ text, speed = 20 }: { text: string; speed?: number }) {
  const [displayed, setDisplayed] = useState("");
  const [isTyping, setIsTyping] = useState(false);
  const indexRef = useRef(0);

  useEffect(() => {
    if (indexRef.current >= text.length) {
      setDisplayed(text);
      setIsTyping(false);
      return;
    }

    setIsTyping(true);
    let i = indexRef.current;

    const interval = setInterval(() => {
      i++;
      setDisplayed(text.slice(0, i));
      indexRef.current = i;
      if (i >= text.length) {
        clearInterval(interval);
        setIsTyping(false);
      }
    }, speed);

    return () => clearInterval(interval);
  }, [text, speed]);

  return (
    <span>
      {displayed}
      {isTyping && (
        <span className="inline-block w-1.5 h-3 bg-primary animate-pulse ml-0.5 align-middle" />
      )}
    </span>
  );
}

export const ResultsSection = ({
  lastTranscription,
  lastAIResponse,
  isAIProcessing,
  isProcessing,
  capturing,
  conversation,
  liveChunks,
  sentBoundary,
  scrollAreaRef,
}: Props) => {
  const hasResponse = lastAIResponse || isAIProcessing;
  const hasLiveChunks = liveChunks.length > 0;
  const hasHistory = conversation.messages.length > 2;
  const showRightPane = hasResponse || liveChunks.some((c) => c.finalized);
  const latestChunkId = liveChunks.length > 0 ? liveChunks[liveChunks.length - 1].id : -1;

  const transcriptEndRef = useRef<HTMLDivElement>(null);
  const aiEndRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    transcriptEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [liveChunks.length]);

  useEffect(() => {
    aiEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [lastAIResponse]);

  const isEmpty = !hasResponse && !lastTranscription && !hasLiveChunks && !capturing;
  if (isEmpty) return null;

  return (
    <div className="flex h-full" ref={scrollAreaRef}>
      {/* Left: Transcript */}
      <div className={cn("flex flex-col min-h-0", showRightPane ? "w-1/2" : "w-full")}>
        <div className="flex-shrink-0 px-3 py-1.5 border-b border-border/30 flex items-center gap-1.5">
          <HeadphonesIcon className="h-3 w-3 text-primary" />
          <span className="text-[10px] font-medium text-primary uppercase tracking-wide">
            Transcript
          </span>
          {isProcessing && (
            <Loader2 className="h-2.5 w-2.5 animate-spin text-muted-foreground" />
          )}
          {capturing && !isProcessing && (
            <span className="text-[9px] text-muted-foreground animate-pulse">
              listening
            </span>
          )}
        </div>

        <div className="flex-1 overflow-y-auto p-3 space-y-1.5">
          {liveChunks.length > 0 ? (
            <>
              {liveChunks.map((chunk, idx) => {
                const isSent = chunk.id < sentBoundary;
                const isLatest = chunk.id === latestChunkId;

                return (
                  <span key={chunk.id}>
                    {idx > 0 && " "}
                    <span
                      className={cn(
                        "text-[11px] leading-relaxed",
                        isSent && "bg-primary/10 rounded px-0.5 text-muted-foreground",
                        isLatest && !isSent && "text-foreground font-medium"
                      )}
                    >
                      {isLatest && !isSent ? (
                        <TypingText text={chunk.text} />
                      ) : (
                        chunk.text
                      )}
                    </span>
                  </span>
                );
              })}
              <div ref={transcriptEndRef} />
            </>
          ) : lastTranscription ? (
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              {lastTranscription}
            </p>
          ) : capturing ? (
            <p className="text-[11px] text-muted-foreground/50 italic animate-pulse">
              Waiting for speech...
            </p>
          ) : null}

          {hasHistory && (
            <div className="border-t border-border/30 pt-2 mt-2 space-y-1">
              <p className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">
                Previous
              </p>
              {conversation.messages
                .slice(2)
                .sort((a, b) => b.timestamp - a.timestamp)
                .map((message, index) => (
                  <div
                    key={message.id || index}
                    className={cn(
                      "p-1.5 rounded-md text-[10px]",
                      message.role === "user"
                        ? "bg-primary/5 border-l-2 border-primary/30"
                        : "bg-background/50"
                    )}
                  >
                    <span className="text-[8px] font-medium text-muted-foreground uppercase">
                      {message.role === "user" ? "System" : "AI"}
                    </span>
                    <div className="text-muted-foreground leading-relaxed mt-0.5 line-clamp-2">
                      {message.content}
                    </div>
                  </div>
                ))}
            </div>
          )}
        </div>
      </div>

      {/* Divider */}
      {showRightPane && <div className="w-px bg-border/50 flex-shrink-0" />}

      {/* Right: AI Response */}
      {showRightPane && (
        <div className="flex flex-col min-h-0 w-1/2">
          <div className="flex-shrink-0 px-3 py-1.5 border-b border-border/30 flex items-center justify-between">
            <div className="flex items-center gap-1.5">
              <BotIcon className="h-3 w-3 text-muted-foreground" />
              <span className="text-[10px] font-medium text-muted-foreground uppercase tracking-wide">
                AI
              </span>
            </div>
            {lastAIResponse && <CopyButton content={lastAIResponse} />}
          </div>

          <div className="flex-1 overflow-y-auto p-3">
            {isAIProcessing && !lastAIResponse ? (
              <div className="flex items-center gap-2 py-4 justify-center">
                <Loader2 className="h-4 w-4 animate-spin text-primary" />
                <span className="text-xs text-muted-foreground">
                  Generating...
                </span>
              </div>
            ) : lastAIResponse ? (
              <div className="prose prose-sm max-w-none dark:prose-invert text-[11px]">
                <Markdown>{lastAIResponse}</Markdown>
                {isAIProcessing && (
                  <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-1 align-middle" />
                )}
                <div ref={aiEndRef} />
              </div>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
};
