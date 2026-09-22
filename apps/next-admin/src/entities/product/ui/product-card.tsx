import Image from 'next/image';
import type { Product } from '../model';

interface ProductCardProps {
  product: Product;
  className?: string;
}

export function ProductCard({ product, className }: ProductCardProps) {
  return (
    <div className={`overflow-hidden rounded-lg border ${className ?? ''}`}>
      <div className='relative aspect-square'>
        <Image
          src={product.photo_url}
          alt={product.name}
          fill
          className='object-cover'
          sizes='(max-width: 768px) 100vw, 33vw'
        />
      </div>
      <div className='p-3'>
        <p className='truncate text-sm font-medium'>{product.name}</p>
        <p className='text-muted-foreground text-xs'>{product.category}</p>
        <p className='mt-1 text-sm font-semibold'>
          ${product.price.toFixed(2)}
        </p>
      </div>
    </div>
  );
}
