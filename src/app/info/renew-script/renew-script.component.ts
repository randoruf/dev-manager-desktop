import {Component, Inject, OnInit} from '@angular/core';
import {NgbActiveModal} from '@ng-bootstrap/ng-bootstrap';
import {Device} from '../../types';
import {DeviceManagerService} from '../../core/services';
import {BehaviorSubject, noop, Observable, Subject} from 'rxjs';
import {save as showSaveDialog} from '@tauri-apps/api/dialog'

@Component({
  selector: 'app-renew-script',
  templateUrl: './renew-script.component.html',
  styleUrls: ['./renew-script.component.scss']
})
export class RenewScriptComponent implements OnInit {

  public decryptedPrivKey?: string;
  public devModeToken$: Observable<string>;
  private devModeTokenSubject: Subject<string>;

  constructor(
    public modal: NgbActiveModal,
    private deviceManager: DeviceManagerService,
    @Inject('device') public device: Device
  ) {
    this.devModeTokenSubject = new BehaviorSubject<string>("");
    this.devModeToken$ = this.devModeTokenSubject.asObservable();
  }

  async ngOnInit(): Promise<void> {
    this.devModeTokenSubject.next(await this.deviceManager.devModeToken(this.device));
    this.decryptedPrivKey = (await this.deviceManager.loadPrivKey(this.device)).privatePEM;
  }

  async copyScript(content: string): Promise<void> {
    await navigator.clipboard.writeText(content);
  }

  saveScript(content: string): void {
    showSaveDialog({
      defaultPath: `renew-devmode-${this.device.name}.sh`
    }).then(value => {
      // this.electron.fs.writeFileSync(value.filePath, content, { encoding: 'utf8', mode: 0o755 });
    }).catch(noop);
  }
}
