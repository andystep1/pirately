import { Header } from "@/components";
import { UseSettingsReturn } from "@/types";
import { Providers } from "./Providers";
import { CustomProviders } from "./CustomProvider";
import { ModelManager } from "../model-manager";

export const STTProviders = (settings: UseSettingsReturn) => {
  return (
    <div id="stt-providers" className="space-y-3">
      <Header
        title="STT Providers"
        description="Select your preferred STT service provider to get started."
        isMainTitle
      />

      <CustomProviders {...settings} />
      <Providers {...settings} />
      <ModelManager
        selectedSttProvider={settings.selectedSttProvider?.provider || ""}
      />
    </div>
  );
};
