import { SwapCreated, SwapRedeemed, SwapRefunded } from "../generated/YonodexHTLC/YonodexHTLC";
import { HTLCSwap } from "../generated/schema";

export function handleSwapCreated(event: SwapCreated): void {
  let swap = new HTLCSwap(event.params.swapId.toHexString());
  swap.sender = event.params.sender;
  swap.receiver = event.params.receiver;
  swap.token = event.params.token;
  swap.amount = event.params.amount;
  swap.hashlock = event.params.hashlock;
  swap.timelock = event.params.timelock;
  swap.status = "CREATED";
  swap.save();
}

export function handleSwapRedeemed(event: SwapRedeemed): void {
  let swap = HTLCSwap.load(event.params.swapId.toHexString());
  if (swap) {
    swap.status = "REDEEMED";
    swap.preimage = event.params.preimage;
    swap.save();
  }
}

export function handleSwapRefunded(event: SwapRefunded): void {
  let swap = HTLCSwap.load(event.params.swapId.toHexString());
  if (swap) {
    swap.status = "REFUNDED";
    swap.save();
  }
}