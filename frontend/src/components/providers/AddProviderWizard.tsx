import { useState } from 'react'
import { useForm } from 'react-hook-form'
import { zodResolver } from '@hookform/resolvers/zod'
import { z } from 'zod'
import { Eye, EyeOff, Loader2 } from 'lucide-react'
import { useCreateProvider, useTestConnection } from '@/hooks/useProviderMutations'
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
  DialogTrigger,
} from '@/components/ui/dialog'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { MultiSelect } from '@/components/ui/multi-select'

const PROVIDER_KINDS = [
  { value: 'openai', label: 'OpenAI' },
  { value: 'anthropic', label: 'Anthropic' },
  { value: 'gemini', label: 'Gemini' },
  { value: 'ollama', label: 'Ollama' },
  { value: 'custom', label: 'Custom' },
] as const

const DEFAULT_BASE_URLS: Record<string, string> = {
  openai: 'https://api.openai.com/v1',
  anthropic: 'https://api.anthropic.com/v1',
  gemini: 'https://generativelanguage.googleapis.com/v1',
  ollama: 'http://localhost:11434',
  custom: '',
}

const KNOWN_MODELS: Record<string, { value: string; label: string }[]> = {
  openai: [
    { value: 'gpt-4o', label: 'GPT-4o' },
    { value: 'gpt-4o-mini', label: 'GPT-4o Mini' },
    { value: 'gpt-4-turbo', label: 'GPT-4 Turbo' },
    { value: 'gpt-3.5-turbo', label: 'GPT-3.5 Turbo' },
  ],
  anthropic: [
    { value: 'claude-3-opus', label: 'Claude 3 Opus' },
    { value: 'claude-3-sonnet', label: 'Claude 3 Sonnet' },
    { value: 'claude-3-haiku', label: 'Claude 3 Haiku' },
  ],
  gemini: [
    { value: 'gemini-pro', label: 'Gemini Pro' },
    { value: 'gemini-ultra', label: 'Gemini Ultra' },
  ],
  ollama: [
    { value: 'llama3', label: 'Llama 3' },
    { value: 'mistral', label: 'Mistral' },
    { value: 'codellama', label: 'CodeLlama' },
  ],
  custom: [],
}

const wizardSchema = z.object({
  name: z.string().min(1, 'Name is required').max(128),
  kind: z.enum(['openai', 'anthropic', 'gemini', 'ollama', 'custom']),
  api_key: z.string().optional(),
  base_url: z.string().url('Must be a valid URL').optional().or(z.literal('')),
  models: z.array(z.string()),
  health_check_interval_seconds: z.coerce.number().min(10).max(3600).default(30),
  health_check_timeout_seconds: z.coerce.number().min(1).max(300).default(10),
  health_check_model: z.string().optional(),
  weight: z.coerce.number().min(1).max(1000).default(100),
  priority: z.coerce.number().min(1).max(100).default(50),
})

type WizardForm = z.infer<typeof wizardSchema>

const STEPS = ['Kind', 'Credentials', 'Models', 'Health Check', 'Routing', 'Review']

export function AddProviderWizard() {
  const [open, setOpen] = useState(false)
  const [step, setStep] = useState(0)
  const [showKey, setShowKey] = useState(false)

  const create = useCreateProvider()
  const testConn = useTestConnection()

  const {
    register,
    handleSubmit,
    watch,
    setValue,
    reset,
    formState: { errors },
  } = useForm<WizardForm>({
    resolver: zodResolver(wizardSchema),
    defaultValues: {
      name: '',
      kind: 'openai',
      api_key: '',
      base_url: DEFAULT_BASE_URLS.openai,
      models: [],
      health_check_interval_seconds: 30,
      health_check_timeout_seconds: 10,
      health_check_model: '',
      weight: 100,
      priority: 50,
    },
  })

  const kind = watch('kind')
  const models = watch('models')
  const apiKey = watch('api_key')
  const baseUrl = watch('base_url')

  const onSubmit = async (data: WizardForm) => {
    await create.mutateAsync(data)
    setOpen(false)
    reset()
    setStep(0)
  }

  const handleKindChange = (value: WizardForm['kind']) => {
    setValue('kind', value)
    setValue('base_url', DEFAULT_BASE_URLS[value] || '')
    setValue('models', [])
  }

  const handleTest = async () => {
    await testConn.mutateAsync({
      config: {
        name: watch('name'),
        kind,
        api_key: apiKey,
        base_url: baseUrl || undefined,
      },
    })
  }

  const modelOptions = KNOWN_MODELS[kind] || []

  const nextStep = () => setStep((s) => Math.min(s + 1, STEPS.length - 1))
  const prevStep = () => setStep((s) => Math.max(s - 1, 0))

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>Add Provider</Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Add Provider</DialogTitle>
          <DialogDescription>
            Step {step + 1} of {STEPS.length}: {STEPS[step]}
          </DialogDescription>
        </DialogHeader>

        {create.isError && (
          <div className="rounded-md border border-destructive/50 bg-destructive/10 p-3 text-sm text-destructive">
            {create.error?.message || 'Failed to create provider'}
          </div>
        )}

        <form onSubmit={handleSubmit(onSubmit)} className="space-y-4">
          {step === 0 && (
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="name">Name</Label>
                <Input id="name" {...register('name')} />
                {errors.name && (
                  <p className="text-sm text-destructive">{errors.name.message}</p>
                )}
              </div>
              <div className="space-y-2">
                <Label>Provider Kind</Label>
                <Select value={kind} onValueChange={handleKindChange}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {PROVIDER_KINDS.map((k) => (
                      <SelectItem key={k.value} value={k.value}>
                        {k.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
            </div>
          )}

          {step === 1 && (
            <div className="space-y-4">
              {kind !== 'ollama' && (
                <div className="space-y-2">
                  <Label htmlFor="api_key">API Key</Label>
                  <div className="relative">
                    <Input
                      id="api_key"
                      type={showKey ? 'text' : 'password'}
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
                </div>
              )}
              <div className="space-y-2">
                <Label htmlFor="base_url">Base URL</Label>
                <Input id="base_url" {...register('base_url')} />
                {errors.base_url && (
                  <p className="text-sm text-destructive">{errors.base_url.message}</p>
                )}
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
            </div>
          )}

          {step === 2 && (
            <div className="space-y-2">
              <Label>Models</Label>
              <MultiSelect
                options={modelOptions}
                value={models}
                onChange={(value) => setValue('models', value)}
                placeholder="Select models..."
              />
            </div>
          )}

          {step === 3 && (
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="health_check_interval_seconds">Check Interval (seconds)</Label>
                <Input
                  id="health_check_interval_seconds"
                  type="number"
                  {...register('health_check_interval_seconds')}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="health_check_timeout_seconds">Timeout (seconds)</Label>
                <Input
                  id="health_check_timeout_seconds"
                  type="number"
                  {...register('health_check_timeout_seconds')}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="health_check_model">Health Check Model</Label>
                <Input
                  id="health_check_model"
                  placeholder="e.g. gpt-4o-mini"
                  {...register('health_check_model')}
                />
              </div>
            </div>
          )}

          {step === 4 && (
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="weight">Weight (1-1000)</Label>
                <Input id="weight" type="number" {...register('weight')} />
              </div>
              <div className="space-y-2">
                <Label htmlFor="priority">Priority (1-100)</Label>
                <Input id="priority" type="number" {...register('priority')} />
              </div>
            </div>
          )}

          {step === 5 && (
            <div className="space-y-2 text-sm">
              <p><strong>Name:</strong> {watch('name')}</p>
              <p><strong>Kind:</strong> {kind}</p>
              <p><strong>Base URL:</strong> {baseUrl || '—'}</p>
              <p><strong>Models:</strong> {models.join(', ') || '—'}</p>
              <p><strong>Weight:</strong> {watch('weight')}</p>
              <p><strong>Priority:</strong> {watch('priority')}</p>
            </div>
          )}

          <DialogFooter>
            {step > 0 && (
              <Button type="button" variant="ghost" onClick={prevStep}>
                Back
              </Button>
            )}
            {step < STEPS.length - 1 ? (
              <Button type="button" onClick={nextStep}>
                Next
              </Button>
            ) : (
              <Button type="submit" disabled={create.isPending}>
                {create.isPending ? 'Creating...' : 'Create Provider'}
              </Button>
            )}
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
