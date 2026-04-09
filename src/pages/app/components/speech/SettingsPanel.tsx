import { useState } from "react";
import {
  Label,
  Switch,
  Textarea,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  SelectLabel,
  SelectGroup,
} from "@/components";
import { WandIcon } from "lucide-react";
import {
  PROMPT_TEMPLATES,
  getPromptTemplateById,
} from "@/lib/platform-instructions";

interface SettingsPanelProps {
  useSystemPrompt: boolean;
  setUseSystemPrompt: (value: boolean) => void;
  contextContent: string;
  setContextContent: (content: string) => void;
}

export const SettingsPanel = ({
  useSystemPrompt,
  setUseSystemPrompt,
  contextContent,
  setContextContent,
}: SettingsPanelProps) => {
  const [selectedTemplate, setSelectedTemplate] = useState<string>("");

  const handleTemplateSelection = (templateId: string) => {
    const template = getPromptTemplateById(templateId);
    if (template) {
      setContextContent(template.prompt);
      setSelectedTemplate("");
    }
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between gap-4">
        <div className="flex-1">
          <Label className="text-[10px] font-medium">Use System Prompt</Label>
          <p className="text-[9px] text-muted-foreground mt-0.5">
            {useSystemPrompt
              ? "Using default prompt from settings"
              : "Using custom context below"}
          </p>
        </div>
        <Switch
          checked={useSystemPrompt}
          onCheckedChange={setUseSystemPrompt}
          className="scale-75"
        />
      </div>

      {!useSystemPrompt && (
        <div className="space-y-2">
          <div className="flex justify-end">
            <Select
              value={selectedTemplate}
              onValueChange={handleTemplateSelection}
            >
              <SelectTrigger className="w-auto h-6 text-[10px]">
                <WandIcon className="w-2.5 h-2.5 mr-1" />
                <SelectValue placeholder="Templates" />
              </SelectTrigger>
              <SelectContent>
                <SelectGroup>
                  <SelectLabel className="text-[10px] py-1">
                    Quick-fill
                  </SelectLabel>
                  {PROMPT_TEMPLATES.map((template) => (
                    <SelectItem
                      key={template.id}
                      value={template.id}
                      className="text-[10px]"
                    >
                      {template.name}
                    </SelectItem>
                  ))}
                </SelectGroup>
              </SelectContent>
            </Select>
          </div>
          <Textarea
            placeholder="Enter custom system prompt and context..."
            value={contextContent}
            onChange={(e) => setContextContent(e.target.value)}
            className="min-h-16 resize-none text-[10px]"
          />
        </div>
      )}
    </div>
  );
};
