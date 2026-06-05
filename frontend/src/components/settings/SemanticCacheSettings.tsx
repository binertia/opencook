import { useState } from 'react'
import { useSemanticCacheStats } from '@/hooks/useSemanticCache'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { AlertTriangle, Database, Zap } from 'lucide-react'

export interface SemanticCacheConfig {
  enabled: boolean
  threshold: number
  embedding_model: string
  max_entries_per_org: number
}

interface SemanticCacheSettingsProps {
  orgId: string
  config: SemanticCacheConfig
  onSave: (config: SemanticCacheConfig) => void
  isSaving?: boolean
}

export default function SemanticCacheSettings({
  orgId,
  config,
  onSave,
  isSaving,
}: SemanticCacheSettingsProps) {
  const [enabled, setEnabled] = useState(config.enabled)
  const [threshold, setThreshold] = useState(config.threshold)
  const [maxEntries, setMaxEntries] = useState(config.max_entries_per_org)

  const { data: stats, isLoading: statsLoading } = useSemanticCacheStats(orgId)

  const handleSave = () => {
    onSave({
      enabled,
      threshold,
      embedding_model: 'text-embedding-3-small',
      max_entries_per_org: maxEntries,
    })
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Zap className="h-5 w-5" />
            Semantic Cache
          </CardTitle>
          <CardDescription>
            Enable embedding-based semantic caching to reduce costs on similar queries.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-6">
          {/* Warning */}
          <div className="rounded-md border border-yellow-500/50 bg-yellow-500/10 p-4 text-yellow-800 dark:text-yellow-200">
            <div className="flex items-start gap-2">
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
              <div className="text-sm">
                <p className="font-medium">Additional embedding API costs apply</p>
                <p className="mt-1 text-yellow-700 dark:text-yellow-300">
                  Semantic cache generates embeddings for each query. Approximate cost: $0.02 per 1M tokens with text-embedding-3-small.
                </p>
              </div>
            </div>
          </div>

          {/* Enable toggle */}
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <Label htmlFor="semantic-cache-enabled">Enable Semantic Cache</Label>
              <p className="text-sm text-muted-foreground">
                Store and match responses by semantic similarity
              </p>
            </div>
            <Switch
              id="semantic-cache-enabled"
              checked={enabled}
              onCheckedChange={setEnabled}
            />
          </div>

          {/* Threshold */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <Label htmlFor="threshold">Similarity Threshold</Label>
              <span className="text-sm font-mono">{threshold.toFixed(2)}</span>
            </div>
            <input
              id="threshold"
              type="range"
              min={0.9}
              max={0.99}
              step={0.01}
              value={threshold}
              onChange={(e) => setThreshold(parseFloat(e.target.value))}
              className="w-full accent-primary"
              disabled={!enabled}
            />
            <p className="text-xs text-muted-foreground">
              Higher values require closer semantic matches. Default: 0.97
            </p>
          </div>

          {/* Max entries */}
          <div className="space-y-2">
            <Label htmlFor="max-entries">Max Entries per Organization</Label>
            <Input
              id="max-entries"
              type="number"
              min={1000}
              max={1000000}
              step={1000}
              value={maxEntries}
              onChange={(e) => setMaxEntries(parseInt(e.target.value, 10))}
              disabled={!enabled}
            />
            <p className="text-xs text-muted-foreground">
              Oldest entries are evicted when this limit is reached. Default: 100,000
            </p>
          </div>

          {/* Embedding model */}
          <div className="space-y-2">
            <Label>Embedding Model</Label>
            <div className="rounded-md border bg-muted/50 px-3 py-2 text-sm text-muted-foreground">
              text-embedding-3-small (OpenAI)
            </div>
          </div>

          <Button onClick={handleSave} disabled={isSaving || !enabled}>
            {isSaving ? 'Saving...' : 'Save Cache Settings'}
          </Button>
        </CardContent>
      </Card>

      {/* Stats */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Database className="h-5 w-5" />
            Cache Statistics
          </CardTitle>
        </CardHeader>
        <CardContent>
          {statsLoading ? (
            <p className="text-sm text-muted-foreground">Loading stats...</p>
          ) : stats ? (
            <div className="grid grid-cols-2 gap-4">
              <div className="rounded-md border p-3">
                <p className="text-sm text-muted-foreground">Total Entries</p>
                <p className="text-2xl font-bold">{stats.total_entries.toLocaleString()}</p>
              </div>
              <div className="rounded-md border p-3">
                <p className="text-sm text-muted-foreground">Newest Entry</p>
                <p className="text-sm font-medium">
                  {stats.newest_entry
                    ? new Date(stats.newest_entry).toLocaleString()
                    : 'No entries'}
                </p>
              </div>
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              Semantic cache stats unavailable. Enable semantic cache to see statistics.
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
