import { useState } from 'react'
import { Check, ChevronDown, X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'

interface MultiSelectProps {
  options: { value: string; label: string }[]
  value: string[]
  onChange: (value: string[]) => void
  placeholder?: string
  disabled?: boolean
}

export function MultiSelect({
  options,
  value,
  onChange,
  placeholder = 'Select options...',
  disabled,
}: MultiSelectProps) {
  const [open, setOpen] = useState(false)

  const toggleOption = (optionValue: string) => {
    if (value.includes(optionValue)) {
      onChange(value.filter((v) => v !== optionValue))
    } else {
      onChange([...value, optionValue])
    }
  }

  const removeOption = (optionValue: string) => {
    onChange(value.filter((v) => v !== optionValue))
  }

  return (
    <div className="space-y-2">
      <DropdownMenu open={open} onOpenChange={setOpen}>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            className="w-full justify-between"
            disabled={disabled}
          >
            <span className="truncate">
              {value.length > 0 ? `${value.length} selected` : placeholder}
            </span>
            <ChevronDown className="ml-2 h-4 w-4 shrink-0 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-56">
          {options.map((option) => (
            <DropdownMenuItem
              key={option.value}
              onSelect={(e) => {
                e.preventDefault()
                toggleOption(option.value)
              }}
              className="flex items-center gap-2"
            >
              <div
                className={cn(
                  'flex h-4 w-4 items-center justify-center rounded border',
                  value.includes(option.value)
                    ? 'border-primary bg-primary text-primary-foreground'
                    : 'border-input'
                )}
              >
                {value.includes(option.value) && <Check className="h-3 w-3" />}
              </div>
              <span>{option.label}</span>
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>

      {value.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {value.map((v) => {
            const label = options.find((o) => o.value === v)?.label || v
            return (
              <span
                key={v}
                className="inline-flex items-center gap-1 rounded-md bg-secondary px-2 py-1 text-xs text-secondary-foreground"
              >
                {label}
                <button
                  type="button"
                  onClick={() => removeOption(v)}
                  className="rounded-sm hover:bg-secondary-foreground/20"
                  disabled={disabled}
                >
                  <X className="h-3 w-3" />
                </button>
              </span>
            )
          })}
        </div>
      )}
    </div>
  )
}
