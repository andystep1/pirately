import { Header } from "@/components";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, useCallback } from "react";
import { AlertCircleIcon, RefreshCwIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ModelCard, ModelInfo } from "./ModelCard";

interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  progress_pct: number;
}

interface ModelManagerProps {
  selectedSttProvider: string;
}

export const ModelManager = ({ selectedSttProvider }: ModelManagerProps) => {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [downloadingIds, setDownloadingIds] = useState<Set<string>>(new Set());
  const [progressMap, setProgressMap] = useState<
    Map<string, DownloadProgress>
  >(new Map());
  const [activeModelId, setActiveModelId] = useState<string | null>(null);

  const fetchModels = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const result = await invoke<ModelInfo[]>("list_available_models");
      setModels(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchModels();
  }, [fetchModels]);

  useEffect(() => {
    const unlisten = listen<DownloadProgress>(
      "model-download-progress",
      (event) => {
        const { model_id, progress_pct } = event.payload;
        setProgressMap((prev) => {
          const next = new Map(prev);
          next.set(model_id, event.payload);
          return next;
        });
        if (progress_pct >= 100) {
          setDownloadingIds((prev) => {
            const next = new Set(prev);
            next.delete(model_id);
            return next;
          });
          setModels((prev) =>
            prev.map((m) =>
              m.id === model_id ? { ...m, is_downloaded: true } : m
            )
          );
          setProgressMap((prev) => {
            const next = new Map(prev);
            next.delete(model_id);
            return next;
          });
        }
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleDownload = async (modelId: string) => {
    try {
      setDownloadingIds((prev) => new Set(prev).add(modelId));
      await invoke("download_model", { modelId });
    } catch (err) {
      setDownloadingIds((prev) => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleCancelDownload = async (modelId: string) => {
    try {
      await invoke("cancel_model_download", { modelId });
      setDownloadingIds((prev) => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
      setProgressMap((prev) => {
        const next = new Map(prev);
        next.delete(modelId);
        return next;
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleDelete = async (modelId: string) => {
    try {
      await invoke("delete_model", { modelId });
      setModels((prev) =>
        prev.map((m) =>
          m.id === modelId ? { ...m, is_downloaded: false } : m
        )
      );
      if (activeModelId === modelId) {
        setActiveModelId(null);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleSelect = async (modelId: string) => {
    try {
      await invoke("set_local_model", { modelId });
      setActiveModelId(modelId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const isInProcess = selectedSttProvider === "in-process";

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <Header
          title="Local Transcription Models"
          description="Download and manage models for in-process speech-to-text."
          isMainTitle
        />
        <Button
          size="icon"
          variant="ghost"
          onClick={fetchModels}
          disabled={loading}
          title="Refresh models"
        >
          <RefreshCwIcon
            className={`h-4 w-4 ${loading ? "animate-spin" : ""}`}
          />
        </Button>
      </div>

      {!isInProcess && (
        <div className="flex items-start gap-2 rounded-lg border border-yellow-500/30 bg-yellow-500/5 p-3">
          <AlertCircleIcon className="h-4 w-4 text-yellow-500 mt-0.5 shrink-0" />
          <p className="text-xs text-muted-foreground">
            Select <strong>In-Process (Local Model)</strong> as your STT
            provider to use local transcription models.
          </p>
        </div>
      )}

      {error && (
        <div className="flex items-start gap-2 rounded-lg border border-destructive/30 bg-destructive/5 p-3">
          <AlertCircleIcon className="h-4 w-4 text-destructive mt-0.5 shrink-0" />
          <p className="text-xs text-destructive">{error}</p>
        </div>
      )}

      {loading && models.length === 0 ? (
        <div className="text-center py-8 text-sm text-muted-foreground">
          Loading models...
        </div>
      ) : models.length === 0 ? (
        <div className="text-center py-8 text-sm text-muted-foreground">
          No models available.
        </div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-2">
          {models.map((model) => (
            <ModelCard
              key={model.id}
              model={model}
              isActive={activeModelId === model.id}
              isDownloading={downloadingIds.has(model.id)}
              progress={progressMap.get(model.id) || null}
              onDownload={handleDownload}
              onCancelDownload={handleCancelDownload}
              onDelete={handleDelete}
              onSelect={handleSelect}
            />
          ))}
        </div>
      )}
    </div>
  );
};
