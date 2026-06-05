import { useEffect } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Edit2, Loader2 } from 'lucide-react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import type { ApiKey } from '@/hooks/useApiKeys'

const SCOPE_OPTIONS = [
  { value: 'chat:write', label: 'Chat Completions' },
  { value: 'embeddings:write', label: 'Embeddings' },
  { value: 'models:read', label: 'List Models' },
  { value: 'usage:read', label: 'Read Usage' },
]

const editKeySchema = z.object({
  name: z.string().min(1, 'Name is required').max(128, 'Max 128 characters'),
  scopes: z.array(z.string()).min(1, 'Select at least one scope'),
  rate_limit_rps: z.coerce.number().int().min(1).max(10000).default(10),
  expires_at: z.string().optional(),
})

type EditKeyForm = z.infer<typeof editKeySchema>

interface EditKeyModalProps {
  apiKey: ApiKey | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (keyId: string, data: { name?: string; status?: string; scopes?: string[]; rate_limit_rps?: number; expires_at?: string }) => void
  isPending: boolean
}

export function EditKeyModal({ apiKey, open, onOpenChange, onSubmit, isPending }: EditKeyModalProps) {
  const {
    register,
    handleSubmit,
    formState: { errors },
    reset,
    setValue,
    watch,
  } = useForm<EditKeyForm>({
    resolver: zodResolver(editKeySchema),
    defaultValues: {
      name: '',
      scopes: ['chat:write'],
      rate_limit_rps: 10,
      expires_at: '',
    },
  })

  const selectedScopes = watch('scopes')

  useEffect(() => {
    if (apiKey) {
      reset({
        name: apiKey.name,
        scopes: apiKey.scopes || ['all'],
        rate_limit_rps: apiKey.rate_limit_rps || 10,
        expires_at: apiKey.expires_at || '',
      })
    }
  }, [apiKey, reset])

  const handleFormSubmit = (data: EditKeyForm) => {
    if (!apiKey) return
    const payload: { name?: string; scopes?: string[]; rate_limit_rps?: number; expires_at?: string } = {
      name: data.name,
      scopes: data.scopes,
      rate_limit_rps: data.rate_limit_rps,
    }
    if (data.expires_at) {
      payload.expires_at = data.expires_at
    }
    onSubmit(apiKey.id, payload)
  }

  const toggleScope = (scope: string) => {
    const current = selectedScopes || []
    if (current.includes(scope)) {
      setValue('scopes', current.filter((s) => s !== scope), { shouldValidate: true })
    } else {
      setValue('scopes', [...current, scope], { shouldValidate: true })
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => { if (!v) reset(); onOpenChange(v) }}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Edit2 className="h-5 w-5" />
            Edit API Key
          </DialogTitle>
          <DialogDescription>
            Update configuration for <strong>{apiKey?.name}</strong>.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit(handleFormSubmit)} className="space-y-4">
          <div className="space-y-2">
            <Label>Prefix</Label>
            <Input value={apiKey?.prefix || ''} disabled className="font-mono text-xs bg-muted" />
            <p className="text-xs text-muted-foreground">Key prefix cannot be changed.</p>
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-name">
              Name <span className="text-destructive">*</span>
            </Label>
            <Input id="edit-name" placeholder="Key name" {...register('name')} />
            {errors.name && (
              <p className="text-sm text-destructive">{errors.name.message}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label>
              Scopes <span className="text-destructive">*</span>
            </Label>
            <div className="flex flex-wrap gap-2">
              {SCOPE_OPTIONS.map((scope) => {
                const isSelected = selectedScopes?.includes(scope.value)
                return (
                  <Button
                    key={scope.value}
                    type="button"
                    variant={isSelected ? 'default' : 'outline'}
                    size="sm"
                    onClick={() => toggleScope(scope.value)}
                  >
                    {scope.label}
                  </Button>
                )
              })}
            </div>
            {errors.scopes && (
              <p className="text-sm text-destructive">{errors.scopes.message}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-rate-limit">Rate Limit (requests/second)</Label>
            <Input
              id="edit-rate-limit"
              type="number"
              min={1}
              max={10000}
              {...register('rate_limit_rps')}
            />
            {errors.rate_limit_rps && (
              <p className="text-sm text-destructive">{errors.rate_limit_rps.message}</p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="edit-expires-at">Expiration (optional)</Label>
            <Input
              id="edit-expires-at"
              type="datetime-local"
              {...register('expires_at')}
            />
          </div>

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button type="submit" disabled={isPending}>
              {isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Save Changes
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
