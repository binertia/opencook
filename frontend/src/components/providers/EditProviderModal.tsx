import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Eye, EyeOff, Loader2 } from 'lucide-react'
import { useUpdateProvider, useTestConnection } from '@/hooks/useProviderMutations'
import type { Provider } from '@/hooks/useProviders'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { MultiSelect } from '@/components/ui/multi-select'
import { Badge } from '@/components/ui/badge'

const editSchema = z.object({
  name: z.string().min(1, 'Name is required').max(128),
  api_key: z.string().optional(),
  base_url: z.string().url('Must be a valid URL').optional().or(z.literal('')),
  models: z.array(z.string()),
  health_check_interval_seconds: z.coerce.number().min(10).max(3600),
  health_check_timeout_seconds: z.coerce.number().min(1).max(300),
  health_check_model: z.string().optional(),
  weight: z.coerce.number().min(1).max(1000),
  priority: z.coerce.number().min(1).max(100),
  status: z.enum(['active', 'inactive']),
})

type EditForm = z.infer<typeof editSchema>

interface EditProviderModalProps {
  provider: Provider | null
  open: boolean
  onOpenChange: (open: boolean) => void
}

const MODEL_OPTIONS = [
  { value: 'gpt-4o', label: 'GPT-4o' },
  { value: 'gpt-4o-mini', label: 'GPT-4o Mini' },
  { value: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
  { value: 'claude-3-opus', label: 'Claude 3 Opus' },
  { value: 'claude-3-sonnet', label: 'Claude 3 Sonnet' },
  { value: 'claude-3-haiku', label: 'Claude 3 Haiku' },
  { value: 'gemini-pro', label: 'Gemini Pro' },
  { value: 'gemini-ultra', label: 'Gemini Ultra' },
  { value: 'llama3', label: 'Llama 3' },
  { value: 'mistral', label: 'Mistral' },
]

export function EditProviderModal({ provider, open, onOpenChange }: EditProviderModalProps) {
  const [showKey, setShowKey] = useState(false)
  const update = useUpdateProvider(provider?.id || '')
  const testConn = useTestConnection()

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    reset,
    formState: { errors, isDirty },
  } = useForm<EditForm>({
    resolver: zodResolver(editSchema),
    values: provider
      ? {
          name: provider.name,
          api_key: '',
          base_url: provider.base_url || '',
          models: [],
          health_check_interval_seconds: 30,
          health_check_timeout_seconds: 10,
          health_check_model: '',
          weight: 100,
          priority: 50,
          status: provider.status as 'active' | 'inactive',
        }
      : undefined,
  })

  const onSubmit = async (data: EditForm) => {
    const payload: Record<string, unknown> = {}
    if (data.name) payload.name = data.name
    if (data.api_key) payload.api_key = data.api_key
    if (data.base_url) payload.base_url = data.base_url
    if (data.models.length > 0) payload.models = data.models
    if (data.health_check_interval_seconds) payload.health_check_interval_seconds = data.health_check_interval_seconds
    if (data.health_check_timeout_seconds) payload.health_check_timeout_seconds = data.health_check_timeout_seconds
    if (data.health_check_model) payload.health_check_model = data.health_check_model
    if (data.weight) payload.weight = data.weight
    if (data.priority) payload.priority = data.priority
    if (data.status) payload.status = data.status

    await update.mutateAsync(payload)
    onOpenChange(false)
    reset()
  }

  const handleTest = async () => {
    if (!provider) return
    await testConn.mutateAsync({
      providerId: provider.id,
      config: {
        name: watch('name'),
        kind: provider.kind as 'openai' | 'anthropic' | 'gemini' | 'ollama' | 'custom',
        api_key: watch('api_key') || undefined,
        base_url: watch('base_url') || undefined,
      },
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Edit Provider</DialogTitle>
          <DialogDescription>
            {provider && (
              <span className="flex items-center gap-2">
                {provider.name}
                <Badge variant="outline" className="capitalize">
                  {provider.kind}
                </Badge>
              </span>
            )}
          </DialogDescription>
        </DialogHeader>

        {update.isError && (
          <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
            {update.error?.message || 'Failed to update provider'}
          </div>
        )}

        <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="edit-name">Name</Label>
            <Input id="edit-name" {...register('name')} />
            {errors.name && (
              <p className="text-sm text-destructive">{errors.name.message}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-api_key">API Key</Label>
            <div className="relative">
              <Input
                id="edit-api_key"
                type={showKey ? 'text' : 'password'}
                placeholder="***"
                {...register('api_key')}
              />
              <Button
                type="button"
                variant="ghost"
                size="icon"
                className="absolute right-0 top-0"
                onClick={() => setShowKey((s) => !s)}
              >
                {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </Button>
            </div>
            <p className="text-xs text-muted-foreground">
              Leave blank to keep existing key.
            </p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-base_url">Base URL</Label>
            <Input id="edit-base_url" {...register('base_url')} />
            {errors.base_url && (
              <p className="text-sm text-destructive">{errors.base_url.message}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label>Models</Label>
            <MultiSelect
              options={MODEL_OPTIONS}
              value={watch('models')}
              onChange={(value) => setValue('models', value, { shouldDirty: true })}
              placeholder="Select models..."
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div className="space-y-2">
              <Label htmlFor="edit-weight">Weight</Label>
              <Input id="edit-weight" type="number" {...register('weight')} />
            </div>
            <div className="space-y-2">
              <Label htmlFor="edit-priority">Priority</Label>
              <Input id="edit-priority" type="number" {...register('priority')} />
            </div>
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-status">Status</Label>
            <select
              id="edit-status"
              className="flex h-10 w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
              {...register('status')}
            >
              <option value="active">Active</option>
              <option value="inactive">Inactive</option>
            </select>
          </div>

          <Button
            type="button"
            variant="outline"
            onClick={handleTest}
            disabled={testConn.isPending}
          >
            {testConn.isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
            Test Connection
          </Button>
          {testConn.isSuccess && (
            <p className="text-sm text-green-600 dark:text-green-400">
              Connected! Latency: {testConn.data.latency_ms}ms
            </p>
          )}
          {testConn.isError && (
            <p className="text-sm text-destructive">
              Connection failed: {testConn.error?.message}
            </p>
          )}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={!isDirty || update.isPending}>
              {update.isPending ? 'Saving...' : 'Save Changes'}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
