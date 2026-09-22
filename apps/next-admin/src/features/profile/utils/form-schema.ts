import * as z from 'zod';

export const profileSchema = z.object({
  firstname: z
    .string()
    .min(3, { message: 'First name must be at least 3 characters' }),
  lastname: z
    .string()
    .min(3, { message: 'Last name must be at least 3 characters' }),
  email: z.string().email({ message: 'Enter a valid email address' }),
  contactno: z.coerce.number(),
  country: z.string().min(1, { message: 'Please select a country' }),
  city: z.string().min(1, { message: 'Please select a city' }),
  // jobs array is for the dynamic fields
  jobs: z.array(
    z.object({
      jobcountry: z.string().min(1, { message: 'Please select a country' }),
      jobcity: z.string().min(1, { message: 'Please select a city' }),
      jobtitle: z
        .string()
        .min(3, { message: 'Job title must be at least 3 characters' }),
      employer: z
        .string()
        .min(3, { message: 'Employer must be at least 3 characters' }),
      startdate: z
        .string()
        .refine((value) => /^\d{4}-\d{2}-\d{2}$/.test(value), {
          message: 'Start date should be in the format YYYY-MM-DD'
        }),
      enddate: z.string().refine((value) => /^\d{4}-\d{2}-\d{2}$/.test(value), {
        message: 'End date should be in the format YYYY-MM-DD'
      })
    })
  )
});

export type ProfileFormValues = z.infer<typeof profileSchema>;
