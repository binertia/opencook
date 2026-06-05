import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { KeyRound, Loader2 } from 'lucide-react'
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import type { CreateApiKeyRequest } from '@/hooks/useApiKeys'

const SCOPE_OPTIONS = [
  { value: 'chat:write', label: 'Chat Completions' },
  { value: 'embeddings:write', label: 'Embeddings' },
  { value: 'models:read', label: 'List Models' },
  { value: 'usage:read', label: 'Read Usage' },
]

const createKeySchema = z.object({
  name: z.string().min(1, 'Name is required').max(128, 'Max 128 characters'),
  scopes: z.array(z.string()).min(1, 'Select at least one scope'),
  rate_limit_rps: z.coerce.number().int().min(1).max(10000).default(10),
  expires_at: z.string().optional(),
})

type CreateKeyForm = z.infer<typeof createKeySchema>

interface CreateKeyModalProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  onSubmit: (data: CreateApiKeyRequest) => void
  isPending: boolean
}

export function CreateKeyModal({ open, onOpenChange, onSubmit, isPending }: CreateKeyModalProps) {
  const {
    register,
    handleSubmit,
    formState: { errors },
    reset,
    setValue,
    watch,
  } = useForm<CreateKeyForm>({
    resolver: zodResolver(createKeySchema),
    defaultValues: {
      name: '',
      scopes: ['chat:write'],
      rate_limit_rps: 10,
      expires_at: '',
    },
  })

  const selectedScopes = watch('scopes')

  const handleFormSubmit = (data: CreateKeyForm) => {
    const payload: CreateApiKeyRequest = {
      name: data.name,
      scopes: data.scopes,
      rate_limit_rps: data.rate_limit_rps,
      expires_at: data.expires_at || undefined,
    }
    onSubmit(payload)
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
            <KeyRound className="h-5 w-5" />
            Create API Key
          </DialogTitle>
          <DialogDescription>
            Configure your new API key. The full key will be shown exactly once after creation.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit(handleFormSubmit)} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="key-name">
              Name <span className="text-destructive">*</span>
            </Label>
            <Input
              id="key-name"
              placeholder="e.g. Production API Key"
              {...register('name')}
            />
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
            <Label htmlFor="rate-limit">Rate Limit (requests/second)</Label>
            <Input
              id="rate-limit"
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
            <Label htmlFor="expires-at">Expiration (optional)</Label>
            <Input
              id="expires-at"
              type="datetime-local"
              {...register('expires_at')}
            />
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={isPending}>
              {isPending && <Loader2 className="mr-2 h-4 w-4 animate-spin" />}
              Create Key
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
