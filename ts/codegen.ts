import codegen from '@cosmwasm/ts-codegen';

codegen({
  contracts: [
    {
      name: 'Sg721Base',
      dir: '../contracts/sg721-base/schema',
    },
    {
      name: 'Sg721MetadataOnchain',
      dir: '../contracts/sg721-metadata-onchain/schema',
    },
    {
      name: 'Sg721Nt',
      dir: '../contracts/sg721-nt/schema',
    },
    {
      name: 'Splits',
      dir: '../contracts/splits/schema',
    },
    {
      name: 'SerialPrintFactory',
      dir: '../contracts/serial-print-factory/schema',
    },
    {
      name: 'SerialPrintMinter',
      dir: '../contracts/serial-print-minter/schema',
    },
  ],
  outPath: './src/',

  // options are completely optional ;)
  options: {
    bundle: {
      bundleFile: 'index.ts',
      scope: 'contracts',
    },
    types: {
      enabled: true,
    },
    client: {
      enabled: true,
    },
    reactQuery: {
      enabled: false,
      optionalClient: true,
      version: 'v4',
      mutations: true,
      queryKeys: true,
    },
    recoil: {
      enabled: false,
    },
    messageComposer: {
      enabled: true,
    },
  },
}).then(() => {
  console.log('✨ all done!');
});
