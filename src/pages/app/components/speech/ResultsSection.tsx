import { LiveChunk } from "@/hooks/useSystemAudio";
import { ChatConversation } from "@/types";
import { Markdown, CopyButton } from "@/components";
import { BotIcon, Loader2, HeadphonesIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { useEffect, useState, useRef } from "react";

type Props = {
  lastTranscription: string;
  lastAIResponse: string;
  isAIProcessing: boolean;
  isProcessing: boolean;
  conversation: ChatConversation;
  liveChunks: LiveChunk[];
  sentBoundary: number;
};

function TypingText({ text, speed = 25 }: { text: string; speed?: number }) {
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
    const startIdx = indexRef.current;
    let i = startIdx;

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
  conversation,
  liveChunks,
  sentBoundary,
}: Props) => {
  const hasResponse = lastAIResponse || isAIProcessing;
  const hasLiveChunks = liveChunks.length > 0;
  const hasHistory = conversation.messages.length > 2;

  if (!hasResponse && !lastTranscription && !hasLiveChunks) {
    return null;
  }

  const transcriptText =
    liveChunks.length > 0
      ? liveChunks.map((c) => c.text).join(" ")
      : lastTranscription;

  const latestChunkId = liveChunks.length > 0 ? liveChunks[liveChunks.length - 1].id : -1;

  return (
    <div className="rounded-lg border border-border/50 bg-muted/20 p-3 space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <HeadphonesIcon className="w-3.5 h-3.5 text-primary" />
          <h4 className="text-xs font-medium">Live Session</h4>
          {isProcessing && !isAIProcessing && (
            <span className="text-[9px] text-muted-foreground animate-pulse ml-1">
              listening...
            </span>
          )}
        </div>
        {lastAIResponse && <CopyButton content={lastAIResponse} />}
      </div>

      <div className="flex gap-3 min-h-0">
        <div className="flex-1 min-w-0 space-y-1.5">
          <div className="flex items-center gap-1 mb-1">
            <HeadphonesIcon className="h-2.5 w-2.5 text-primary" />
            <span className="text-[9px] font-medium text-primary uppercase tracking-wide">
              Transcript
            </span>
            {isProcessing && (
              <Loader2 className="h-2.5 w-2.5 animate-spin text-muted-foreground" />
            )}
          </div>

          <div className="space-y-0.5 text-[11px] leading-relaxed max-h-60 overflow-y-auto">
            {liveChunks.length > 0 ? (
              liveChunks.map((chunk, idx) => {
                const isSent = chunk.id < sentBoundary;
                const isLatest = chunk.id === latestChunkId;

                return (
                  <span key={chunk.id}>
                    {idx > 0 && " "}
                    <span
                      className={cn(
                        isSent && "bg-primary/10 rounded px-0.5",
                        isLatest && !isSent && "text-foreground font-medium"
                      )}
                    >
                      {isLatest && !isSent ? (
                        <TypingText text={chunk.text} speed={20} />
                      ) : (
                        chunk.text
                      )}
                    </span>
                  </span>
                );
              })
            ) : (
              transcriptText && (
                <span className="text-muted-foreground">{transcriptText}</span>
              )
            )}

            {isProcessing && liveChunks.length === 0 && (
              <span className="text-muted-foreground/50 italic">
                Waiting for speech...
              </span>
            )}
          </div>
        </div>

        {(hasResponse || liveChunks.some((c) => c.finalized)) && (
          <div className="w-px bg-border/50 flex-shrink-0" />
        )}

        {(hasResponse || liveChunks.some((c) => c.finalized)) && (
          <div className="flex-1 min-w-0 space-y-1.5">
            <div className="flex items-center gap-1 mb-1">
              <BotIcon className="h-2.5 w-2.5 text-muted-foreground" />
              <span className="text-[9px] font-medium text-muted-foreground uppercase tracking-wide">
                AI
              </span>
            </div>

            {isAIProcessing && !lastAIResponse ? (
              <div className="flex items-center gap-2">
                <Loader2 className="h-3.5 w-3.5 animate-spin text-primary" />
                <span className="text-[10px] text-muted-foreground">
                  Generating...
                </span>
              </div>
            ) : lastAIResponse ? (
              <div className="prose prose-sm max-w-none dark:prose-invert text-[11px] max-h-60 overflow-y-auto">
                <Markdown>{lastAIResponse}</Markdown>
                {isAIProcessing && (
                  <span className="inline-block w-2 h-4 bg-primary animate-pulse ml-1 align-middle" />
                )}
              </div>
            ) : null}
          </div>
        )}
      </div>

      {hasHistory && (
        <div className="border-t border-border/50 pt-2 space-y-1">
          <p className="text-[9px] text-muted-foreground uppercase tracking-wide">
            Previous
          </p>
          <div className="space-y-1 max-h-32 overflow-y-auto">
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
                  <div className="text-muted-foreground leading-relaxed mt-0.5">
                    <Markdown>{message.content}</Markdown>
                  </div>
                </div>
              ))}
          </div>
        </div>
      )}
    </div>
  );
};
