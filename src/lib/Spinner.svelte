<script lang="ts">
	import type { SpinnerTypes } from '../types/spinner.type';
	export let color: SpinnerTypes['color'] = '#ffc131';
	export let unit: SpinnerTypes['unit'] = 'px';
	export let duration: SpinnerTypes['duration'] = '0.75s';
	export let size: SpinnerTypes['size'] = '20';
	export let pause: SpinnerTypes['pause'] = false;

</script>

<div
	class="spinner"
	class:pause-animation={pause}
	style="--size: {size}{unit}; --color: {color}; --duration: {duration};"
/>

<style>
	.spinner {
		height: var(--size);
		width: var(--size);
		border-color: var(--color) transparent var(--color) var(--color);
		border-width: calc(var(--size) / 8);
		border-style: solid;
		border-image: initial;
		border-radius: 50%;
		animation: var(--duration) linear 0s infinite normal none running rotate;
		background-color: #2b2b2b;		
	}
	.pause-animation {
		animation-play-state: paused;
	}
	@keyframes rotate {
		0% {
			transform: rotate(0);
		}
		100% {
			transform: rotate(360deg);
		}
	}
</style>