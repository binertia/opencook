import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Switch } from '@/components/ui/switch'
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table'
import type { ProviderModel } from '@/hooks/useProviders'

interface ModelListProps {
  models: ProviderModel[]
  onToggle: (modelId: string, enabled: boolean) => void
  isUpdating: boolean
}

export function ModelList({ models, onToggle, isUpdating }: ModelListProps) {
  const [pendingId, setPendingId] = useState<string | null>(null)

  const handleToggle = (model: ProviderModel) => {
    if (isUpdating) return
    setPendingId(model.id)
    onToggle(model.id, model.status !== 'active')
  }

  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Model ID</TableHead>
            <TableHead>Name</TableHead>
            <TableHead>Status</TableHead>
            <TableHead>Input Cost</TableHead>
            <TableHead>Output Cost</TableHead>
            <TableHead>Capabilities</TableHead>
            <TableHead className="w-16">Enabled</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {models.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={7}
                className="text-center text-muted-foreground"
              >
                No models configured.
              </TableCell>
            </TableRow>
          ) : (
            models.map((model) => (
              <TableRow key={model.id}>
                <TableCell className="font-mono text-xs">{model.id}</TableCell>
                <TableCell className="font-medium">{model.name}</TableCell>
                <TableCell>
                  <Badge
                    variant={model.status === 'active' ? 'default' : 'secondary'}
                  >
                    {model.status}
                  </Badge>
                </TableCell>
                <TableCell>
                  {model.pricing ? (
                    <span className="text-sm">
                      ${model.pricing.input_per_1m_tokens.toFixed(2)} / 1M tokens
                    </span>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </TableCell>
                <TableCell>
                  {model.pricing ? (
                    <span className="text-sm">
                      ${model.pricing.output_per_1m_tokens.toFixed(2)} / 1M tokens
                    </span>
                  ) : (
                    <span className="text-muted-foreground">—</span>
                  )}
                </TableCell>
                <TableCell>
                  <div className="flex flex-wrap gap-1">
                    {(model.capabilities || []).map((cap) => (
                      <Badge key={cap} variant="outline" className="text-xs">
                        {cap}
                      </Badge>
                    ))}
                  </div>
                </TableCell>
                <TableCell>
                  <Switch
                    checked={model.status === 'active'}
                    onCheckedChange={() => handleToggle(model)}
                    disabled={isUpdating && pendingId === model.id}
                    aria-label={`Toggle ${model.name}`}
                  />
                </TableCell>
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  )
}
