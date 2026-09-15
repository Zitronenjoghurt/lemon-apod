<script lang="ts" setup>
import { computed, type Component } from 'vue'
import {
  AlignLeft,
  ArrowDownLeft,
  ArrowLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowRightLeft,
  ArrowUp,
  ArrowUpRight,
  AtSign,
  Bell,
  BookOpen,
  Cake,
  Calendar,
  CalendarClock,
  CalendarOff,
  ChartColumn,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ChevronsLeft,
  ChevronsRight,
  CircleAlert,
  CircleCheck,
  CircleHelp,
  CirclePlus,
  CircleSlash,
  CircleX,
  Clock,
  Copy,
  Database,
  Download,
  Eclipse,
  Equal,
  Eraser,
  ExternalLink,
  Eye,
  FastForward,
  File,
  Flag,
  Gamepad2,
  Globe,
  Heart,
  Hourglass,
  House,
  Image,
  ImageOff,
  Images,
  Info,
  LayoutGrid,
  List,
  LoaderCircle,
  Mail,
  MapPin,
  Menu,
  Moon,
  MoonStar,
  Newspaper,
  Orbit,
  Pause,
  Pencil,
  Play,
  RefreshCw,
  Repeat,
  Rocket,
  Satellite,
  Search,
  Settings,
  Share2,
  Shield,
  Shuffle,
  SlidersHorizontal,
  Smartphone,
  Sparkles,
  Star,
  Sun,
  Telescope,
  Timer,
  Trash2,
  TriangleAlert,
  Upload,
  User,
  Video,
  Vote,
  Wifi,
  X,
  Zap,
  ZoomIn,
} from 'lucide-vue-next'

const ICONS: Record<string, Component> = {
  absent: CircleSlash,
  'align-left': AlignLeft,
  'arrow-down-left': ArrowDownLeft,
  'arrow-left': ArrowLeft,
  'arrow-right': ArrowRight,
  'arrow-up': ArrowUp,
  'arrow-up-right': ArrowUpRight,
  at: AtSign,
  bell: Bell,
  birthday: Cake,
  bolt: Zap,
  book: BookOpen,
  calendar: Calendar,
  'calendar-clock': CalendarClock,
  'calendar-times': CalendarOff,
  'chart-bar': ChartColumn,
  check: Check,
  'check-circle': CircleCheck,
  'chevron-down': ChevronDown,
  'chevron-left': ChevronLeft,
  'chevron-right': ChevronRight,
  'chevron-up': ChevronUp,
  clock: Clock,
  cog: Settings,
  compare: ArrowRightLeft,
  conjunction: Orbit,
  copy: Copy,
  database: Database,
  download: Download,
  eclipse: Eclipse,
  envelope: Mail,
  equals: Equal,
  eraser: Eraser,
  'exclamation-circle': CircleAlert,
  'exclamation-triangle': TriangleAlert,
  external: ExternalLink,
  eye: Eye,
  feed: Newspaper,
  file: File,
  'first-page': ChevronsLeft,
  flag: Flag,
  forward: FastForward,
  games: Gamepad2,
  globe: Globe,
  grid: LayoutGrid,
  heart: Heart,
  home: House,
  hourglass: Hourglass,
  image: Image,
  'image-lost': ImageOff,
  images: Images,
  imminent: Timer,
  info: Info,
  'last-page': ChevronsRight,
  launch: Rocket,
  list: List,
  menu: Menu,
  mobile: Smartphone,
  moon: Moon,
  'moon-phase': MoonStar,
  orbit: Orbit,
  pause: Pause,
  payload: Satellite,
  pencil: Pencil,
  place: MapPin,
  planets: Telescope,
  play: Play,
  'plus-circle': CirclePlus,
  question: CircleHelp,
  random: Shuffle,
  refresh: RefreshCw,
  replay: Repeat,
  search: Search,
  settings: SlidersHorizontal,
  share: Share2,
  shield: Shield,
  shower: Sparkles,
  sparkles: Sparkles,
  spinner: LoaderCircle,
  star: Star,
  sun: Sun,
  swap: ArrowLeftRight,
  telescope: Telescope,
  times: X,
  'times-circle': CircleX,
  trash: Trash2,
  upload: Upload,
  user: User,
  video: Video,
  vote: Vote,
  wifi: Wifi,
  zoom: ZoomIn,
}

const BRANDS = new Set(['discord', 'github'])

const props = withDefaults(
  defineProps<{
    name: string
    size?: number | string
    label?: string
    spin?: boolean
    fill?: boolean
  }>(),
  { size: '1em', label: undefined, spin: false, fill: false },
)

const component = computed(() => ICONS[props.name])
const brand = computed(() => (BRANDS.has(props.name) ? props.name : null))
</script>

<template>
  <svg
    v-if="brand === 'discord'"
    :aria-hidden="label ? undefined : 'true'"
    :aria-label="label"
    :class="['icon', { spin }]"
    :height="size"
    :role="label ? 'img' : undefined"
    :width="size"
    fill="currentColor"
    viewBox="0 0 24 24"
  >
    <path
      d="M20.317 4.369a19.79 19.79 0 0 0-4.885-1.515.074.074 0 0 0-.79.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.6 12.6 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.32.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .31.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028c.462-.63.874-1.295 1.226-1.994a.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128c.126-.094.252-.192.372-.291a.074.074 0 0 1 .078-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .79.009c.12.099.246.198.373.292a.077.077 0 0 1-.6.127 12.3 12.3 0 0 1-1.873.892.077.077 0 0 0-.41.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .84.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.028ZM8.02 15.331c-1.182 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418Zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418Z"
    />
  </svg>

  <svg
    v-else-if="brand === 'github'"
    :aria-hidden="label ? undefined : 'true'"
    :aria-label="label"
    :class="['icon', { spin }]"
    :height="size"
    :role="label ? 'img' : undefined"
    :width="size"
    fill="currentColor"
    viewBox="0 0 24 24"
  >
    <path
      d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577 0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7 3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07 1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93 0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267 1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24 2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81 2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12"
    />
  </svg>

  <component
    :is="component"
    v-else-if="component"
    :aria-hidden="label ? undefined : 'true'"
    :aria-label="label"
    :class="['icon', { spin }]"
    :fill="fill ? 'currentColor' : 'none'"
    :role="label ? 'img' : undefined"
    :size="size"
  />
</template>

<style scoped>
.icon {
  display: inline-block;
  flex: none;
  align-self: center;
  vertical-align: -0.145em;
  stroke-width: 2px;
}

.spin {
  animation: turn 1s linear infinite;
}

@keyframes turn {
  to {
    transform: rotate(360deg);
  }
}

@media (prefers-reduced-motion: reduce) {
  .spin {
    animation: none;
  }
}
</style>
