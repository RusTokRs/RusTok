import { richTextFrameAssetResponse } from '@rustok/richtext/next';

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ asset: string }> }
) {
  const { asset } = await params;
  return richTextFrameAssetResponse(asset);
}
