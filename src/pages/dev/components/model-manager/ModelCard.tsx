import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardHeader,
  CardTitle,
  CardContent,
  CardFooter,
} from "@/components/ui/card";
import { DownloadIcon, TrashIcon, CheckIcon, XIcon } from "lucide-react";
import { useState } from "react";

export interface ModelInfo {
  id: string;
  name: string;
  engine_type:
    | "Whisper"
    | "Parakeet"
    | "Moonshine"
    | "MoonshineStreaming"
    | "SenseVoice"
    | "GigaAM"
    | "Canary"
    | "Cohere";
  filename: string;
  url: string;
  sha256: string;
  size_mb: number;
  is_directory: boolean;
  is_downloaded: boolean;
}

interface DownloadProgress {
  model_id: string;
  downloaded_bytes: number;
  total_bytes: number;
  progress_pct: number;
}

interface ModelCardProps {
  model: ModelInfo;
  isActive: boolean;
  isDownloading: boolean;
  progress: DownloadProgress | null;
  onDownload: (modelId: string) => void;
  onCancelDownload: (modelId: string) => void;
  onDelete: (modelId: string) => void;
  onSelect: (modelId: string) => void;
}

export const ModelCard = ({
  model,
  isActive,
  isDownloading,
  progress,
  onDownload,
  onCancelDownload,
  onDelete,
  onSelect,
}: ModelCardProps) => {
  const [confirmDelete, setConfirmDelete] = useState(false);

  const statusText = isDownloading
    ? "Downloading..."
    : model.is_downloaded
    ? "Downloaded"
    : "Not Downloaded";

  const statusVariant = isDownloading
    ? "secondary"
    : model.is_downloaded
    ? "default"
    : "outline";

  return (
    <Card className="p-3 border !bg-transparent border-input/50 gap-3">
      <CardHeader className="p-0 px-1">
        <div className="flex items-center justify-between">
          <CardTitle className="text-sm font-medium">{model.name}</CardTitle>
          <Badge variant="outline" className="text-[10px]">
            {model.engine_type}
          </Badge>
        </div>
      </CardHeader>

      <CardContent className="p-0 space-y-2">
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>{model.size_mb.toFixed(1)} MB</span>
          <span>•</span>
          <Badge variant={statusVariant} className="text-[10px] px-1.5 py-0">
            {statusText}
          </Badge>
          {isActive && (
            <Badge
              variant="default"
              className="text-[10px] px-1.5 py-0 bg-green-600"
            >
              Active
            </Badge>
          )}
        </div>

        {isDownloading && progress && (
          <div className="space-y-1">
            <div className="w-full h-1.5 bg-secondary rounded-full overflow-hidden">
              <div
                className="h-full bg-primary rounded-full transition-all duration-300"
                style={{ width: `${Math.min(progress.progress_pct, 100)}%` }}
              />
            </div>
            <p className="text-[10px] text-muted-foreground">
              {progress.progress_pct.toFixed(1)}%
            </p>
          </div>
        )}
      </CardContent>

      <CardFooter className="p-0 gap-1">
        {isDownloading ? (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => onCancelDownload(model.id)}
            className="text-destructive hover:text-destructive h-7 text-xs"
          >
            <XIcon className="h-3 w-3" />
            Cancel
          </Button>
        ) : model.is_downloaded ? (
          <>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => onSelect(model.id)}
              disabled={isActive}
              className="h-7 text-xs"
            >
              <CheckIcon className="h-3 w-3" />
              {isActive ? "Selected" : "Select"}
            </Button>
            {!confirmDelete ? (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setConfirmDelete(true)}
                className="text-destructive hover:text-destructive h-7 text-xs"
              >
                <TrashIcon className="h-3 w-3" />
                Delete
              </Button>
            ) : (
              <>
                <Button
                  size="sm"
                  variant="destructive"
                  onClick={() => {
                    onDelete(model.id);
                    setConfirmDelete(false);
                  }}
                  className="h-7 text-xs"
                >
                  Confirm
                </Button>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => setConfirmDelete(false)}
                  className="h-7 text-xs"
                >
                  Cancel
                </Button>
              </>
            )}
          </>
        ) : (
          <Button
            size="sm"
            variant="outline"
            onClick={() => onDownload(model.id)}
            className="h-7 text-xs"
          >
            <DownloadIcon className="h-3 w-3" />
            Download
          </Button>
        )}
      </CardFooter>
    </Card>
  );
};
